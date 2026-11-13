//! Compiles a function of the code editor with the compiler of devkitARM.
//!
//! The function is standalone: it calls no function of the plugin or of a library, and it keeps no address of its own data
//! in a global. The compiler makes position-independent code, the linker puts the code, the constants and the data one after
//! the other from the address 0, and the result goes in a file that the plugin can load anywhere in memory.
//!
//! The function that the plugin calls is `extern "C" u32 sango_run(u32 argument)`: its name is [`ENTRY`].
//!
//! # The file of a compiled function (`SCOD`)
//!
//! All the numbers are little-endian. The header has 64 bytes, then the image.
//!
//! | Bytes | Content |
//! | --- | --- |
//! | 0-3 | The signature `SCOD`. |
//! | 4-7 | The version, 1. |
//! | 8-11 | The place of the entry function in the image, in bytes (the image starts at 0). |
//! | 12-15 | The size of the image, in bytes. |
//! | 16-19 | The size of the zeros after the image (the data that starts at 0), in bytes. |
//! | 20-23 | The flags, 0. |
//! | 24-63 | The name of the function, text, filled with zeros. |
//!
//! The image is ARM code (the instruction set of the 3DS), then the constants, then the data that has a value. The plugin
//! copies the image to a memory that it can run, puts zeros after it (the size of the bytes 16-19), flushes the caches, and
//! calls the address of the image plus the entry place, with the argument in `r0`. The result is in `r0`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The name of the function that the plugin calls.
pub const ENTRY: &str = "sango_run";

/// The signature of a compiled function file.
pub const MAGIC: &[u8; 4] = b"SCOD";

/// The version of the file.
pub const VERSION: u32 = 1;

/// The size of the header of the file, in bytes.
pub const HEADER_SIZE: usize = 64;

/// The linker script: the code, the constants, the data and the zeros, from the address 0.
const LINKER_SCRIPT: &str = "ENTRY(sango_run)
SECTIONS {
  . = 0;
  .text : { KEEP(*(.text.sango_run)) *(.text .text.*) }
  .rodata : { *(.rodata .rodata.*) }
  .data : { *(.data .data.*) }
  .bss : { __sango_bss_start = .; *(.bss .bss.* COMMON) __sango_bss_end = .; }
  /DISCARD/ : { *(.ARM.exidx*) *(.ARM.extab*) *(.comment) *(.note*) *(.ARM.attributes) }
}
";

/// The options of the compiler: the processor of the 3DS, code that does not depend on its place, and the options of the
/// plugin (no exceptions, no RTTI).
const FLAGS: [&str; 15] = [
    "-march=armv6k",
    "-mtune=mpcore",
    "-mfloat-abi=hard",
    "-mtp=soft",
    "-O2",
    "-fPIC",
    "-ffunction-sections",
    "-fdata-sections",
    "-fno-strict-aliasing",
    "-fno-rtti",
    "-fno-exceptions",
    "-fno-threadsafe-statics",
    "-std=gnu++11",
    "-D__3DS__",
    "-DUSE_SANGO_PLUGIN",
];

/// The relocations that cannot work in an image that the plugin puts anywhere: they need the address of the image.
const FORBIDDEN_RELOCATIONS: [&str; 6] = ["R_ARM_ABS32", "R_ARM_GOT", "R_ARM_TARGET1", "R_ARM_MOVW_ABS", "R_ARM_MOVT_ABS", "R_ARM_PLT32"];

/// The programs and the folders that a compilation uses.
#[derive(Clone, Debug)]
pub struct Toolchain {
    pub gxx: PathBuf,
    pub objcopy: PathBuf,
    pub nm: PathBuf,
    pub readelf: PathBuf,
    /// The folders of the headers.
    pub includes: Vec<PathBuf>,
}

/// How serious a message of the compiler is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// A message of the compiler about a line of the code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The line (from 1), or 0 when the message is not about a line.
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub message: String,
}

/// What a compilation gave.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub diagnostics: Vec<Diagnostic>,
    /// The whole text that the programs wrote, for the user who wants to read it.
    pub log: String,
    /// The file of the compiled function, when the compilation worked.
    pub file: Option<Vec<u8>>,
}

impl Output {
    /// True when the compilation gave a file.
    pub fn is_ok(&self) -> bool {
        self.file.is_some()
    }

    /// The number of messages of a kind.
    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == severity).count()
    }
}

impl Toolchain {
    /// Finds the programs in the folder of devkitPro. The headers are the ones of `libctru` and `libctrpf` of devkitPro,
    /// and `lib/include` of the Sango plugin when its folder is given.
    pub fn new(devkitpro: &Path, plugin: Option<&Path>) -> Result<Toolchain, String> {
        let gxx = sango_core::compiler_in(devkitpro).ok_or("The folder of devkitPro has no devkitARM/bin/arm-none-eabi-g++.")?;
        let bin = gxx.parent().ok_or("The compiler has no folder.")?.to_path_buf();
        let program = |name: &str| -> PathBuf {
            let exe = bin.join(format!("{name}.exe"));
            if exe.is_file() {
                exe
            } else {
                bin.join(name)
            }
        };
        let mut includes = Vec::new();
        if let Some(plugin) = plugin {
            includes.push(plugin.join("lib").join("include"));
            includes.push(plugin.join("lib").join("src"));
        }
        includes.push(devkitpro.join("libctru").join("include"));
        includes.push(devkitpro.join("libctrpf").join("include"));
        Ok(Toolchain { gxx, objcopy: program("arm-none-eabi-objcopy"), nm: program("arm-none-eabi-nm"), readelf: program("arm-none-eabi-readelf"), includes })
    }

    /// A command of a program of devkitARM, without a console window on Windows.
    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        if let Some(bin) = program.parent() {
            // The programs find each other by the PATH.
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths = vec![bin.to_path_buf()];
            paths.extend(std::env::split_paths(&path));
            if let Ok(joined) = std::env::join_paths(paths) {
                command.env("PATH", joined);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
    }

    /// Runs a program and gives what it wrote (the output and the errors), and whether it worked.
    fn run(&self, command: &mut Command) -> Result<(String, bool), String> {
        let result = command.output().map_err(|error| format!("The program did not start: {error}"))?;
        let mut text = String::from_utf8_lossy(&result.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&result.stderr));
        Ok((text, result.status.success()))
    }

    /// Compiles the source of a function. `name` names the function in the file and in the temporary folder.
    pub fn compile(&self, name: &str, source: &str) -> Output {
        let mut output = Output::default();
        let folder = std::env::temp_dir().join(format!("sango_code_{}_{}", std::process::id(), sanitize(name)));
        let result = self.compile_in(&folder, name, source, &mut output);
        let _ = std::fs::remove_dir_all(&folder);
        if let Err(message) = result {
            output.diagnostics.push(Diagnostic { line: 0, column: 0, severity: Severity::Error, message });
            output.file = None;
        }
        output
    }

    fn compile_in(&self, folder: &Path, name: &str, source: &str, output: &mut Output) -> Result<(), String> {
        std::fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
        let source_path = folder.join("function.cc");
        let object = folder.join("function.o");
        let elf = folder.join("function.elf");
        let image_path = folder.join("function.bin");
        let script = folder.join("function.ld");
        std::fs::write(&source_path, source).map_err(|error| format!("{}: {error}", source_path.display()))?;
        std::fs::write(&script, LINKER_SCRIPT).map_err(|error| format!("{}: {error}", script.display()))?;

        // 1. Compile.
        let mut command = self.command(&self.gxx);
        command.current_dir(folder).args(FLAGS).args(self.includes.iter().map(|path| format!("-I{}", path.display()))).arg("-DGAME_ORAS").arg("-c").arg("function.cc").arg("-o").arg("function.o");
        let (log, worked) = self.run(&mut command)?;
        output.log.push_str(&log);
        output.diagnostics.extend(parse_diagnostics(&log));
        if !worked || !object.is_file() {
            return explain_failure(output, "The compiler stopped without a message.");
        }

        // 2. The code must not need the address of its own data: the plugin puts it anywhere.
        let mut command = self.command(&self.readelf);
        command.current_dir(folder).arg("-r").arg("function.o");
        let (relocations, _) = self.run(&mut command)?;
        if let Some(bad) = forbidden_relocation(&relocations) {
            output.diagnostics.push(Diagnostic {
                line: 0,
                column: 0,
                severity: Severity::Error,
                message: format!("The code needs the address of something in memory ({bad}): a global that holds a pointer, a virtual function or a table of strings. A standalone function cannot do that yet: use values and local data."),
            });
            return Ok(());
        }

        // 3. Link, from the address 0, with no library.
        let mut command = self.command(&self.gxx);
        command.current_dir(folder).args(["-march=armv6k", "-mtune=mpcore", "-mfloat-abi=hard", "-nostdlib", "-nostartfiles", "-Wl,-T,function.ld", "-Wl,--gc-sections", "-Wl,--no-warn-rwx-segments", "function.o", "-o", "function.elf"]);
        let (log, worked) = self.run(&mut command)?;
        output.log.push_str(&log);
        if !worked || !elf.is_file() {
            output.diagnostics.extend(link_diagnostics(&log));
            return explain_failure(output, "The linker stopped without a message.");
        }

        // 4. The places of the entry function and of the zeros, and the image.
        let mut command = self.command(&self.nm);
        command.current_dir(folder).arg("function.elf");
        let (symbols, _) = self.run(&mut command)?;
        let place = |symbol: &str| symbols.lines().find_map(|line| {
            let mut parts = line.split_whitespace();
            let (address, _kind, found) = (parts.next()?, parts.next()?, parts.next()?);
            (found == symbol).then(|| u32::from_str_radix(address, 16).ok()).flatten()
        });
        let entry = place(ENTRY).ok_or_else(|| format!("The code has no function {ENTRY}: write `extern \"C\" u32 {ENTRY}(u32 argument)`."))?;
        let bss = place("__sango_bss_end").unwrap_or(0).saturating_sub(place("__sango_bss_start").unwrap_or(0));
        let mut command = self.command(&self.objcopy);
        command.current_dir(folder).args(["-O", "binary", "function.elf", "function.bin"]);
        let (log, worked) = self.run(&mut command)?;
        if !worked {
            output.log.push_str(&log);
            return Err("objcopy could not make the image.".into());
        }
        let image = std::fs::read(&image_path).map_err(|error| format!("{}: {error}", image_path.display()))?;
        if image.is_empty() {
            return Err("The image is empty.".into());
        }
        output.file = Some(container(name, entry, &image, bss));
        Ok(())
    }
}

/// When the programs failed without a message that was understood, the whole text is the message.
fn explain_failure(output: &mut Output, fallback: &str) -> Result<(), String> {
    if output.count(Severity::Error) == 0 {
        let text = output.log.trim();
        output.diagnostics.push(Diagnostic { line: 0, column: 0, severity: Severity::Error, message: if text.is_empty() { fallback.to_string() } else { text.chars().take(600).collect() } });
    }
    Ok(())
}

/// A name that is safe in the name of a folder.
fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

/// The first relocation that cannot work in an image that moves, from the text of `readelf -r`.
pub fn forbidden_relocation(text: &str) -> Option<&'static str> {
    text.lines().find_map(|line| {
        let kind = line.split_whitespace().nth(2)?;
        FORBIDDEN_RELOCATIONS.iter().copied().find(|bad| kind.starts_with(bad))
    })
}

/// The messages of the compiler: lines like `function.cc:12:5: error: message`.
pub fn parse_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut found: Vec<Diagnostic> = Vec::new();
    for line in text.lines() {
        // The file name can be a Windows path with `C:`: look for `:<line>:<column>:` after the name.
        let Some(at) = line.find(".cc:").or_else(|| line.find(".h:")).or_else(|| line.find(".cpp:")) else { continue };
        let rest = &line[at..];
        let mut parts = rest.splitn(5, ':');
        let (_extension, line_text, column_text, kind, message) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next());
        let (Some(line_number), Some(column), Some(kind)) = (line_text.and_then(|t| t.trim().parse::<usize>().ok()), column_text.and_then(|t| t.trim().parse::<usize>().ok()), kind) else { continue };
        let severity = match kind.trim() {
            "error" | "fatal error" => Severity::Error,
            "warning" => Severity::Warning,
            "note" => Severity::Note,
            _ => continue,
        };
        // A message about a header is not about a line of the function (the file of the function is `function.cc`).
        let in_function = &line[..at] == "function";
        found.push(Diagnostic { line: if in_function { line_number } else { 0 }, column, severity, message: message.unwrap_or("").trim().to_string() });
    }
    found
}

/// The messages of the linker: a call to something that the code does not have.
pub fn link_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some(at) = line.find("undefined reference to") {
            let name = line[at + "undefined reference to".len()..].trim().trim_matches(|c| c == '`' || c == '\'' || c == '"');
            found.push(Diagnostic { line: 0, column: 0, severity: Severity::Error, message: format!("The code calls {name}, which is not in the code. A standalone function cannot call another function yet: write what it needs in the function.") });
        }
    }
    found
}

/// Makes the file of a compiled function.
pub fn container(name: &str, entry: u32, image: &[u8], bss: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_SIZE + image.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend(VERSION.to_le_bytes());
    bytes.extend(entry.to_le_bytes());
    bytes.extend((image.len() as u32).to_le_bytes());
    bytes.extend(bss.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    let mut field = [0u8; 40];
    let name = name.as_bytes();
    let length = name.len().min(39);
    field[..length].copy_from_slice(&name[..length]);
    bytes.extend_from_slice(&field);
    bytes.extend_from_slice(image);
    bytes
}

/// What the header of a compiled function says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub name: String,
    pub entry: u32,
    pub image_size: u32,
    pub bss_size: u32,
}

/// Reads the header of a compiled function and checks that the file is whole.
pub fn read_container(bytes: &[u8]) -> Result<(Info, &[u8]), String> {
    if bytes.get(..4) != Some(MAGIC) {
        return Err("This is not a compiled function (SCOD).".into());
    }
    let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("The file is too short.");
    if word(4)? != VERSION {
        return Err(format!("The version {} of the file is not {VERSION}.", word(4)?));
    }
    let (entry, image_size, bss_size) = (word(8)?, word(12)?, word(16)?);
    let image = bytes.get(HEADER_SIZE..HEADER_SIZE + image_size as usize).ok_or("The image is shorter than the header says.")?;
    if entry >= image_size {
        return Err("The entry function is out of the image.".into());
    }
    let name_field = &bytes[24..HEADER_SIZE];
    let name = String::from_utf8_lossy(&name_field[..name_field.iter().position(|&b| b == 0).unwrap_or(name_field.len())]).to_string();
    Ok((Info { name, entry, image_size, bss_size }, image))
}

/// The template of a new function.
pub fn template(name: &str) -> String {
    format!(
        "// {name}\n//\n// The plugin runs `{ENTRY}` with a number, and gets a number back.\n// The function is standalone: it cannot call another function yet.\n\n#include \"common.h\"\n\nextern \"C\" u32 {ENTRY}(u32 argument) {{\n  return argument + 1;\n}}\n"
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_messages_of_the_compiler_are_read() {
        let text = "function.cc: In function 'u32 sango_run(u32)':\nfunction.cc:7:3: error: 'x' was not declared in this scope\n    7 |   x = 1;\n      |   ^\nC:/devkitPro/libctru/include/3ds/types.h:20:5: warning: something\nfunction.cc:9:1: warning: unused variable 'y' [-Wunused-variable]\nfunction.cc:9:1: note: declared here\nfunction.cc:11:2: fatal error: nothing.h: No such file or directory";
        let found = parse_diagnostics(text);
        assert_eq!(found.len(), 5);
        assert_eq!((found[0].line, found[0].column, found[0].severity), (7, 3, Severity::Error));
        assert_eq!(found[0].message, "'x' was not declared in this scope");
        assert_eq!(found[1].line, 0, "a message about a header is not about a line of the function");
        assert_eq!(found[2].severity, Severity::Warning);
        assert_eq!(found[3].severity, Severity::Note);
        assert_eq!((found[4].line, found[4].severity), (11, Severity::Error));
    }

    #[test]
    fn a_call_to_nothing_and_an_address_are_refused_with_a_reason() {
        let link = link_diagnostics("ld.exe: function.o: in function `sango_run':\nfunction.cc:(.text.sango_run+0x10): undefined reference to `memcpy'");
        assert_eq!(link.len(), 1);
        assert!(link[0].message.contains("memcpy") && link[0].message.contains("standalone"));
        assert_eq!(forbidden_relocation("Offset Info Type\n0000002c  00000a03 R_ARM_REL32       00000000  x\n00000040  00000b02 R_ARM_ABS32       00000000  y"), Some("R_ARM_ABS32"));
        assert_eq!(forbidden_relocation("0000002c  00000a03 R_ARM_REL32  x\n00000030 00000c1c R_ARM_CALL y"), None);
    }

    #[test]
    fn the_file_of_a_compiled_function_is_written_and_read_back() {
        let file = container("Boost", 8, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 12);
        assert_eq!(file.len(), HEADER_SIZE + 10);
        let (info, image) = read_container(&file).unwrap();
        assert_eq!(info, Info { name: "Boost".into(), entry: 8, image_size: 10, bss_size: 12 });
        assert_eq!(image, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert!(read_container(b"nope").is_err());
        assert!(read_container(&file[..HEADER_SIZE + 3]).is_err());
        let mut far = file.clone();
        far[8] = 200;
        assert!(read_container(&far).unwrap_err().contains("entry"));
    }

    /// With devkitPro on this computer (the folder is in `SANGO_DEVKITPRO`): a function is compiled and has the right
    /// bytes, and mistakes are reported with their line.
    #[test]
    fn a_function_is_compiled_with_devkitarm() {
        let Ok(root) = std::env::var("SANGO_DEVKITPRO") else { return };
        let plugin = std::env::var("SANGO_PLUGIN").ok().map(PathBuf::from);
        let tool = Toolchain::new(Path::new(&root), plugin.as_deref()).unwrap();

        let good = "extern \"C\" unsigned sango_run(unsigned argument) { static const unsigned table[4] = {10, 20, 30, 40}; return table[argument & 3] + 5; }\n";
        let output = tool.compile("Table", good);
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        let (info, image) = read_container(output.file.as_ref().unwrap()).unwrap();
        assert_eq!((info.name.as_str(), info.entry), ("Table", 0));
        assert!(image.len() >= 0x20 && image.len() < 0x100, "{}", image.len());

        // The data that has a value, and the zeros that follow.
        let data = "extern \"C\" unsigned sango_run(unsigned a) { static unsigned count = 7; static unsigned zeros[4]; count += a; zeros[a & 3] = count; return zeros[0] + count; }\n";
        let output = tool.compile("Data", data);
        let (info, _) = read_container(output.file.as_ref().unwrap()).unwrap();
        assert_eq!(info.bss_size, 16);

        let bad = "extern \"C\" unsigned sango_run(unsigned a) {\n  return missing + a;\n}\n";
        let output = tool.compile("Bad", bad);
        assert!(!output.is_ok());
        assert!(output.diagnostics.iter().any(|d| d.line == 2 && d.severity == Severity::Error && d.message.contains("missing")), "{:?}", output.diagnostics);

        let calls = "extern \"C\" void* memcpy(void*, const void*, unsigned long);\nextern \"C\" unsigned sango_run(unsigned a) { unsigned b[2] = {a, 1}; unsigned c[2]; memcpy(c, b, 8); return c[0]; }\n";
        let output = tool.compile("Calls", calls);
        assert!(!output.is_ok() || output.diagnostics.is_empty(), "the compiler may inline memcpy");

        let pointer = "static const char* names[2] = {\"a\", \"b\"};\nextern \"C\" unsigned sango_run(unsigned a) { return names[a & 1][0]; }\n";
        let output = tool.compile("Pointer", pointer);
        let _ = output; // the optimiser may remove the table: the test only needs the compiler to run

        let no_entry = "extern \"C\" unsigned other(unsigned a) { return a; }\n";
        let output = tool.compile("Other", no_entry);
        assert!(!output.is_ok() && output.diagnostics.iter().any(|d| d.message.contains(ENTRY)), "{:?}", output.diagnostics);

        // A header of the plugin compiles when the folder of the plugin is given.
        if plugin.is_some() {
            let header = "#include \"common.h\"\n#include \"pokemon/native/move_data.h\"\nextern \"C\" u32 sango_run(u32 argument) {\n  pokemon::MoveData& data = pokemon::MoveData::GetInstance(static_cast<MoveId>(argument));\n  data.power = 90;\n  return data.power;\n}\n";
            let output = tool.compile("Header", header);
            assert!(output.is_ok(), "{:?}\n{}", output.diagnostics, output.log);
        }
    }
}
