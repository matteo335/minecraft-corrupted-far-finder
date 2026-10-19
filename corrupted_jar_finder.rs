use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

fn u16le(b: &[u8], o: usize) -> usize { u16::from_le_bytes([b[o], b[o + 1]]) as usize }
fn u32le(b: &[u8], o: usize) -> usize { u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize }

// What Java's ZipInputStream.getNextEntry() complains about for one entry
fn entry_bad(d: &[u8], o: usize, name: &str) -> Option<String> {
    if o + 10 > d.len() || u32le(d, o) != 0x04034b50 {
        return Some(format!("entry \"{name}\": broken local header at offset {o}"));
    }
    let (flags, method) = (u16le(d, o + 6), u16le(d, o + 8));
    if flags & 8 != 0 && method != 8 {
        return Some(format!("entry \"{name}\": uncompressed entry with data descriptor."));
    }

    None
}

// Ok(Some(msg)) = the crash condition, Err(msg) = unreadable zip, Ok(None) = fine
fn check(d: &[u8]) -> Result<Option<String>, String> {
    if d.len() < 22 { return Err("file too small".into()); }
    let start = d.len().saturating_sub(22 + 65535);
    let eocd = (start..=d.len() - 22).rev()
        .find(|&i| u32le(d, i) == 0x06054b50)
        .ok_or("no end-of-central-directory (truncated or not a zip)")?;
    let count = u16le(d, eocd + 10);
    let mut p = u32le(d, eocd + 16);
    let mut ents: Vec<(usize, String)> = Vec::new();
    for _ in 0..count {
        if p + 46 > d.len() || u32le(d, p) != 0x02014b50 { return Err("broken central directory".into()); }
        let nl = u16le(d, p + 28);
        if p + 46 + nl > d.len() { return Err("broken central directory".into()); }
        let name = String::from_utf8_lossy(&d[p + 46..p + 46 + nl]).to_string();
        ents.push((u32le(d, p + 42), name));
        p += 46 + nl + u16le(d, p + 30) + u16le(d, p + 32);
    }
    ents.sort(); // file order

    // Mimic java.util.jar.JarInputStream: reads the first entry, skips "META-INF/" if it is first,
    // and after reading META-INF/MANIFEST.MF it reads the header of the entry that follows it.
    let bad_at = |i: usize| ents.get(i).and_then(|(o, n)| entry_bad(d, *o, n));
    if let Some(m) = bad_at(0) { return Ok(Some(m)); }
    let mut cur = 0;
    if ents.first().map_or(false, |e| e.1.eq_ignore_ascii_case("META-INF/")) {
        cur = 1;
        if let Some(m) = bad_at(1) { return Ok(Some(m)); }
    }
    if ents.get(cur).map_or(false, |e| e.1.eq_ignore_ascii_case("META-INF/MANIFEST.MF")) {
        if let Some(m) = bad_at(cur + 1) { return Ok(Some(m)); }
    }
    Ok(None)
}

fn prompt(msg: &str) -> String {
    print!("{msg}");
    let _ = io::stdout().flush();
    let mut s = String::new();
    let _ = io::stdin().read_line(&mut s);
    s.trim().trim_matches('"').to_string()
}

fn jar_files(mods: &Path) -> Vec<PathBuf> {
    let mut jars: Vec<PathBuf> = match fs::read_dir(mods) {
        Ok(rd) => rd.flatten().map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("jar")))
            .collect(),
        Err(_) => Vec::new(),
    };
    jars.sort();
    jars
}

fn scan(mods: &Path) {
    if !mods.is_dir() && !mods.extension().map_or(false, |x| x.eq_ignore_ascii_case("jar")) {
        println!("Cannot to open this script with : {}", mods.display());
    }

    let jars = jar_files(mods);
    println!("\nChecking {} jars in {}", jars.len(), mods.display());
    let mut found = 0;
    for p in jars {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        match fs::read(&p) {
            Err(e) => { println!("UNREADABLE: {name} -> {e}"); found += 1; }
            Ok(d) => match check(&d) {
                Ok(Some(msg)) => { println!("BAD: {name} -> {msg}"); found += 1; }
                Err(msg) => { println!("UNREADABLE: {name} -> {msg}"); found += 1; }
                Ok(None) => {}
            },
        }
    }
    if found == 0 { 
        println!("No broken jar found."); 
    } else if found == 1 {
        println!("Remove this mod to stop your current crash.");
    } else if found > 1 {
        println!("Remove these mods to stop your current crash.");
    }

    pause();
}

fn pause() {
    let quit = prompt("\nDo you want to search another modpack?\nIf you have been told to run this tool by someone else, just press \"Enter\" to close it: ");
    
    if quit.eq_ignore_ascii_case("yes") || quit.eq_ignore_ascii_case("y") {
        main();
    } else {
        std::process::exit(0);
    }
}

fn main() {
    // Dragging a mods folder onto the .exe (or passing it as an argument) skips the menu
    if let Some(arg) = env::args().nth(1) {
        scan(Path::new(&arg));
        pause();
        return;
    }

    let windows_default_path = PathBuf::from(env::var("USERPROFILE").unwrap_or_default()).join("curseforge").join("minecraft").join("Instances");

    let windows_c_path = PathBuf::from("C:").join("curseforge").join("minecraft").join("instances");
    let windows_d_path = PathBuf::from("D:").join("curseforge").join("minecraft").join("instances");
    let windows_e_path = PathBuf::from("E:").join("curseforge").join("minecraft").join("instances");

    let mac_path = PathBuf::from("");
    let linux_path = PathBuf::from("");
    
    let resolved_path;

    if windows_default_path.is_dir() {
        resolved_path = windows_default_path;
    } else if windows_c_path.is_dir() {
        resolved_path = windows_c_path;
    } else if windows_d_path.is_dir() {
        resolved_path = windows_d_path;
    } else if windows_e_path.is_dir() {
        resolved_path = windows_e_path;
    } else if mac_path.is_dir() {
        resolved_path = mac_path;
    } else if linux_path.is_dir() {
        resolved_path = linux_path;
    } else {
        println!("Instances folder not found!");
        resolved_path = PathBuf::from(prompt("Type the path of your instances folder: "));
    }


    loop {
        // every instance that has a mods folder
        let mut insts: Vec<PathBuf> = match fs::read_dir(&resolved_path) {
            Ok(rd) => rd.flatten().map(|e| e.path()).filter(|p| p.join("mods").is_dir()).collect(),
            Err(e) => { println!("Cannot read {}: {e}", resolved_path.display()); pause(); return; }
        };
        insts.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if insts.is_empty() {
            println!("No instance with a mods folder in {}", resolved_path.display());
            pause();
            return;
        }

        println!("\nInstances in {}:\n", resolved_path.display());
        for (i, p) in insts.iter().enumerate() {
            let jar_number = jar_files(&p.join("mods")).len();

            println!("  {:>2}. {}  ({} jars)", i + 1, p.file_name().unwrap().to_string_lossy(), jar_number);
        }

        let entry = prompt("\nSelect the number corresponding to your modpack: ");
        
        match entry.parse::<usize>() {
            Ok(jar_number) if jar_number >= 1 && jar_number <= insts.len() => {
                println!("\n=== {} ===", insts[jar_number - 1].file_name().unwrap().to_string_lossy());
                
                scan(&insts[jar_number - 1].join("mods"));
            }
            _ => println!("Invalid choice."),
        }
    }
}
