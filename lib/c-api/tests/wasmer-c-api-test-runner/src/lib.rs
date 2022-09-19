#[cfg(test)]
use std::error::Error;

#[cfg(test)]
static INCLUDE_REGEX: &str = "#include \"(.*)\"";

#[derive(Debug)]
pub struct Config {
    pub wasmer_dir: String,
    pub root_dir: String,
    // linux + mac
    pub cflags: String,
    pub ldflags: String,
    pub ldlibs: String,
    // windows msvc
    pub msvc_cflags: String,
    pub msvc_ldflags: String,
    pub msvc_ldlibs: String,
}

impl Config {
    pub fn get() -> Config {
        let mut config = Config {
            wasmer_dir: std::env::var("WASMER_DIR").unwrap_or_default(),
            root_dir: std::env::var("ROOT_DIR").unwrap_or_default(),

            cflags: std::env::var("CFLAGS").unwrap_or_default(),
            ldflags: std::env::var("LDFLAGS").unwrap_or_default(),
            ldlibs: std::env::var("LDLIBS").unwrap_or_default(),

            msvc_cflags: std::env::var("MSVC_CFLAGS").unwrap_or_default(),
            msvc_ldflags: std::env::var("MSVC_LDFLAGS").unwrap_or_default(),
            msvc_ldlibs: std::env::var("MSVC_LDLIBS").unwrap_or_default(),
        };

        let wasmer_base_dir = find_wasmer_base_dir();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        if config.wasmer_dir.is_empty() {
            println!("manifest dir = {manifest_dir}, wasmer root dir = {wasmer_base_dir}");
            config.wasmer_dir = wasmer_base_dir.clone() + "/package";
            if !std::path::Path::new(&config.wasmer_dir).exists() {
                if !std::path::Path::new(&format!("{wasmer_base_dir}/target/release")).exists() {
                    println!("running make build-capi...");
                    // run make build-capi
                    let mut cmd = std::process::Command::new("make");
                    cmd.arg("build-capi");
                    cmd.current_dir(wasmer_base_dir.clone());
                    let result = cmd.output();
                    println!("make build-capi: {result:#?}");
                }

                println!("running make package...");
                // run make package-capi
                let mut cmd = std::process::Command::new("make");
                cmd.arg("package-capi");
                cmd.current_dir(wasmer_base_dir.clone());
                let result = cmd.output();
                make_package();
                println!("make package: {result:#?}");

                println!("list {}", config.wasmer_dir);
                match std::fs::read_dir(&config.wasmer_dir) {
                    Ok(o) => {
                        for entry in o {
                            let entry = entry.unwrap();
                            let path = entry.path();
                            println!("    {:?}", path.file_name());
                        }
                    }
                    Err(e) => {
                        println!("error in reading config.wasmer_dir: {e}");
                    }
                };

                println!("list {}/include", config.wasmer_dir);
                match std::fs::read_dir(&format!("{}/include", config.wasmer_dir)) {
                    Ok(o) => {
                        for entry in o {
                            let entry = entry.unwrap();
                            let path = entry.path();
                            println!("    {:?}", path.file_name());
                        }
                    }
                    Err(e) => {
                        println!("error in reading config.wasmer_dir: {e}");
                    }
                };
            }
        }
        if config.root_dir.is_empty() {
            config.root_dir = wasmer_base_dir + "/lib/c-api/tests";
        }

        config
    }
}

fn find_wasmer_base_dir() -> String {
    let wasmer_base_dir = env!("CARGO_MANIFEST_DIR");
    let mut path2 = wasmer_base_dir.split("wasmer").collect::<Vec<_>>();
    path2.pop();
    let mut wasmer_base_dir = path2.join("wasmer");

    if wasmer_base_dir.contains("wasmer/lib/c-api") {
        wasmer_base_dir = wasmer_base_dir
            .split("wasmer/lib/c-api")
            .next()
            .unwrap()
            .to_string()
            + "wasmer";
    } else if wasmer_base_dir.contains("wasmer\\lib\\c-api") {
        wasmer_base_dir = wasmer_base_dir
            .split("wasmer\\lib\\c-api")
            .next()
            .unwrap()
            .to_string()
            + "wasmer";
    }

    wasmer_base_dir
}

#[derive(Default)]
pub struct RemoveTestsOnDrop {}

impl Drop for RemoveTestsOnDrop {
    fn drop(&mut self) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        for entry in std::fs::read_dir(&manifest_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let extension = path.extension().and_then(|s| s.to_str());
            if extension == Some("obj") || extension == Some("exe") || extension == Some("o") {
                println!("removing {}", path.display());
                let _ = std::fs::remove_file(&path);
            }
        }
        if let Some(parent) = std::path::Path::new(&manifest_dir).parent() {
            for entry in std::fs::read_dir(&parent).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let extension = path.extension().and_then(|s| s.to_str());
                if extension == Some("obj") || extension == Some("exe") || extension == Some("o") {
                    println!("removing {}", path.display());
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

fn make_package() {
    let wasmer_root_dir = find_wasmer_base_dir();
    let _ = std::fs::create_dir_all(&format!("{wasmer_root_dir}/package/lib"));
    let _ = std::fs::create_dir_all(&format!("{wasmer_root_dir}/package/include"));
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/lib/c-api/tests/wasm.h"),
        &format!("{wasmer_root_dir}/package/include/wasm.h"),
    );
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/lib/c-api/tests/wasmer.h"),
        &format!("{wasmer_root_dir}/package/include/wasmer.h"),
    );
    #[cfg(target_os = "windows")]
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/target/release/wasmer.dll"),
        &format!("{wasmer_root_dir}/package/lib"),
    );
    #[cfg(target_os = "windows")]
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/target/release/wasmer.dll.lib"),
        &format!("{wasmer_root_dir}/package/lib"),
    );
    #[cfg(not(target_os = "windows"))]
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/target/release/libwasmer.so"),
        &format!("{wasmer_root_dir}/package/lib"),
    );
    #[cfg(not(target_os = "windows"))]
    let _ = std::fs::copy(
        &format!("{wasmer_root_dir}/target/release/libwasmer.lib"),
        &format!("{wasmer_root_dir}/package/lib"),
    );
    println!("copying done (make package)");
}

#[cfg(test)]
pub const CAPI_BASE_TESTS: &[&str] = &[
    "wasm-c-api/example/callback",
    "wasm-c-api/example/memory",
    "wasm-c-api/example/start",
    "wasm-c-api/example/global",
    "wasm-c-api/example/reflect",
    "wasm-c-api/example/trap",
    "wasm-c-api/example/hello",
    "wasm-c-api/example/serialize",
    "wasm-c-api/example/multi",
];

#[allow(unused_variables, dead_code)]
pub const CAPI_BASE_TESTS_NOT_WORKING: &[&str] = &[
    "wasm-c-api/example/finalize",
    "wasm-c-api/example/hostref",
    "wasm-c-api/example/threads",
    "wasm-c-api/example/table",
];

// Runs all the tests that are working in the /c directory
#[test]
fn test_ok() {
    let _drop = RemoveTestsOnDrop::default();
    let config = Config::get();
    println!("config: {:#?}", config);

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let host = target_lexicon::HOST.to_string();
    let target = &host;

    let wasmer_dll_dir = format!("{}/lib", config.wasmer_dir);
    let exe_dir = format!("{manifest_dir}/../wasm-c-api/example");
    let path = std::env::var("PATH").unwrap_or_default();
    let newpath = format!("{wasmer_dll_dir};{path}");

    if target.contains("msvc") {
        for test in CAPI_BASE_TESTS.iter() {
            let mut build = cc::Build::new();
            let mut build = build
                .cargo_metadata(false)
                .warnings(true)
                .static_crt(true)
                .extra_warnings(true)
                .warnings_into_errors(false)
                .debug(config.ldflags.contains("-g"))
                .host(&host)
                .target(target)
                .opt_level(1);

            let compiler = build.try_get_compiler().unwrap();
            let mut command = compiler.to_command();

            command.arg(&format!("{manifest_dir}/../{test}.c"));
            if !config.msvc_cflags.is_empty() {
                command.arg(config.msvc_cflags.clone());
            } else if !config.wasmer_dir.is_empty() {
                command.arg("/I");
                command.arg(&format!("{}", config.root_dir));
                command.arg("/I");
                command.arg(&format!("{}/include", config.wasmer_dir));
                let mut log = String::new();
                fixup_symlinks(&[
                    format!("{}/include", config.wasmer_dir),
                    format!("{}", config.root_dir),
                ], &mut log)
                .expect(&format!("failed to fix symlinks: {log}"));
                println!("{log}");
            }
            command.arg("/link");
            if !config.msvc_ldlibs.is_empty() {
                command.arg(config.msvc_ldlibs.clone());
            } else if !config.wasmer_dir.is_empty() {
                command.arg(&format!("/LIBPATH:{}/lib", config.wasmer_dir));
                command.arg(&format!("{}/lib/wasmer.dll.lib", config.wasmer_dir));
            }
            command.arg(&format!("/OUT:\"{manifest_dir}/../{test}.exe\""));

            // run vcvars
            let vcvars_bat_path = find_vcvars64(&compiler).expect("no vcvars64.bat");
            let mut vcvars = std::process::Command::new("cmd");
            vcvars.arg("/C");
            vcvars.arg(vcvars_bat_path);
            println!("running {vcvars:?}");

            // cmd /C vcvars64.bat
            let output = vcvars
                .output()
                .expect("could not invoke vcvars64.bat at {vcvars_bat_path}");

            if !output.status.success() {
                println!();
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                print_wasmer_root_to_stdout(&config);
                panic!("failed to invoke vcvars64.bat {test}");
            }

            println!("compiling {test}: {command:?}");

            // compile
            let output = command
                .output()
                .expect(&format!("failed to compile {command:#?}"));
            if !output.status.success() {
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stdout: {}", String::from_utf8_lossy(&output.stderr));
                print_wasmer_root_to_stdout(&config);
                panic!("failed to compile {test}");
            }

            // execute
            let mut command = std::process::Command::new(&format!("{manifest_dir}/../{test}.exe"));
            command.env("PATH", newpath.clone());
            command.current_dir(exe_dir.clone());
            println!("executing {test}: {command:?}");
            println!("setting current dir = {exe_dir}");
            let output = command
                .output()
                .expect(&format!("failed to run {command:#?}"));
            if !output.status.success() {
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stdout: {}", String::from_utf8_lossy(&output.stderr));
                print_wasmer_root_to_stdout(&config);
                panic!("failed to execute {test}");
            }

            // cc -g -IC:/Users/felix/Development/wasmer/lib/c-api/tests/
            //          -IC:/Users/felix/Development/wasmer/package/include
            //
            //          -Wl,-rpath,C:/Users/felix/Development/wasmer/package/lib
            //
            //          wasm-c-api/example/callback.c
            //
            //          -LC:/Users/felix/Development/wasmer/package/lib -lwasmer
            //
            // -o wasm-c-api/example/callback
        }
    } else {
        for test in CAPI_BASE_TESTS.iter() {
            let compiler_cmd = match std::process::Command::new("cc").output() {
                Ok(_) => "cc",
                Err(_) => "gcc",
            };
            let mut command = std::process::Command::new(compiler_cmd);

            if !config.cflags.is_empty() {
                for f in config.cflags.split_whitespace() {
                    command.arg(f);
                }
            } else if !config.wasmer_dir.is_empty() {
                command.arg("-I");
                command.arg(&config.root_dir);
                command.arg("-I");
                command.arg(&format!("{}/include", config.wasmer_dir));
            }
            if !config.ldflags.is_empty() {
                for f in config.ldflags.split_whitespace() {
                    command.arg(f);
                }
            }
            command.arg(&format!("{manifest_dir}/../{test}.c"));
            if !config.ldlibs.is_empty() {
                for f in config.ldlibs.split_whitespace() {
                    command.arg(f);
                }
            } else if !config.wasmer_dir.is_empty() {
                command.arg(&format!("-L{}/lib", config.wasmer_dir));
                command.arg(&format!("-lwasmer"));
            }
            command.arg("-o");
            command.arg(&format!("{manifest_dir}/../{test}"));

            print_wasmer_root_to_stdout(&config);

            println!("compile: {command:#?}");
            // compile
            let output = command
                .output()
                .expect(&format!("failed to compile {command:#?}"));
            if !output.status.success() {
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                print_wasmer_root_to_stdout(&config);
                panic!("failed to compile {test}: {command:#?}");
            }

            // execute
            let mut command = std::process::Command::new(&format!("{manifest_dir}/../{test}"));
            command.env("PATH", newpath.clone());
            command.current_dir(exe_dir.clone());
            println!("execute: {command:#?}");
            let output = command
                .output()
                .expect(&format!("failed to run {command:#?}"));
            if !output.status.success() {
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                print_wasmer_root_to_stdout(&config);
                panic!("failed to execute {test}: {command:#?}");
            }
        }
    }

    for test in CAPI_BASE_TESTS.iter() {
        let _ = std::fs::remove_file(&format!("{manifest_dir}/{test}.obj"));
        let _ = std::fs::remove_file(&format!("{manifest_dir}/../{test}.exe"));
        let _ = std::fs::remove_file(&format!("{manifest_dir}/../{test}"));
    }
}

#[cfg(test)]
fn print_wasmer_root_to_stdout(config: &Config) {
    println!("print_wasmer_root_to_stdout");

    use walkdir::WalkDir;

    for entry in WalkDir::new(&config.wasmer_dir)
            .into_iter()
            .filter_map(Result::ok) {
        let f_name = String::from(entry.path().canonicalize().unwrap().to_string_lossy());
        println!("{f_name}");
    }

    for entry in WalkDir::new(&config.root_dir)
    .into_iter()
    .filter_map(Result::ok) {
        let f_name = String::from(entry.path().canonicalize().unwrap().to_string_lossy());
        println!("{f_name}");
    }

    println!("printed");
}

#[cfg(test)]
fn fixup_symlinks(include_paths: &[String], log: &mut String) -> Result<(), Box<dyn Error>> {
    log.push_str(&format!("include paths: {include_paths:?}"));
    for i in include_paths {
        let i = i.replacen("-I", "", 1);
        let mut paths_headers = Vec::new();
        let readdir = match std::fs::read_dir(&i) {
            Ok(o) => o,
            Err(_) => continue,
        };
        for entry in readdir {
            let entry = entry?;
            let path = entry.path();
            let path_display = format!("{}", path.display());
            if path_display.ends_with("h") {
                paths_headers.push(path_display);
            }
        }
        fixup_symlinks_inner(&paths_headers, log)?;
    }

    Ok(())
}

#[cfg(test)]
fn fixup_symlinks_inner(include_paths: &[String], log: &mut String) -> Result<(), Box<dyn Error>> {
    log.push_str(&format!("fixup symlinks: {include_paths:#?}"));
    let regex = regex::Regex::new(INCLUDE_REGEX).unwrap();
    for path in include_paths.iter() {
        let file = match std::fs::read_to_string(&path) {
            Ok(o) => o,
            _ => continue,
        };
        // VERY hacky.
        if file.contains("#include \"../wasmer.h\"") {
            std::fs::write(&path, file.replace("#include \"../wasmer.h\"", "#include \"wasmer.h\""))?;
        }
        let lines_3 = file.lines().take(3).collect::<Vec<_>>();
        log.push_str(&format!("first 3 lines of {path:?}: {:#?}\n", lines_3));

        let parent = std::path::Path::new(&path).parent().unwrap();
        if let Ok(symlink) = std::fs::read_to_string(parent.clone().join(&file)) {
            log.push_str(&format!("symlinking {path:?}\n"));
            std::fs::write(&path, symlink)?;
        }

        // follow #include directives and recurse
        let filepaths = regex
            .captures_iter(&file)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>();
        log.push_str(&format!("regex captures: ({path:?}): {:#?}\n", filepaths));
        let joined_filepaths = filepaths
            .iter()
            .filter_map(|s| {
                let path = parent.clone().join(s);
                Some(format!("{}", path.display()))
            })
            .collect::<Vec<_>>();
        fixup_symlinks_inner(&joined_filepaths, log)?;
    }
    Ok(())
}

#[cfg(test)]
fn find_vcvars64(compiler: &cc::Tool) -> Option<String> {
    if !compiler.is_like_msvc() {
        return None;
    }

    let path = compiler.path();
    let path = format!("{}", path.display());
    let split = path.split("VC").nth(0)?;

    Some(format!("{split}VC\\Auxiliary\\Build\\vcvars64.bat"))
}
