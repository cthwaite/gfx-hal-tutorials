extern crate glsl_to_spirv;

use std::path::Path;

use std::error::Error;
use glsl_to_spirv::ShaderType;

static SHADER_PATH : &'static str = "assets/shaders";
static SPIRV_PATH : &'static str = "assets/gen/shaders";

fn main() -> Result<(), Box<Error>> {
    use glsl_to_spirv::ShaderType;

    // Tell the build script to only run again if we change our source shaders
    println!("cargo:rerun-if-changed={}", SHADER_PATH);

    std::fs::create_dir_all(SPIRV_PATH)?;

    for entry in std::fs::read_dir(SHADER_PATH)? {
        let entry = entry?;

        if entry.file_type()?.is_file() {
            let in_path = entry.path();

            let shader_type = in_path.extension().and_then(|ext| {
                match ext.to_string_lossy().as_ref() {
                    "vert" => Some(ShaderType::Vertex),
                    "vs" => Some(ShaderType::Vertex),
                    "frag" => Some(ShaderType::Fragment),
                    "fs" => Some(ShaderType::Fragment),
                    "geom" => Some(ShaderType::Geometry),
                    "gs" => Some(ShaderType::Geometry),
                    _ => None
                }
            });

            if let Some(shader_type) = shader_type {
                use std::io::Read;

                let source = std::fs::read_to_string(&in_path)?;
                let mut compiled_file = glsl_to_spirv::compile(&source, shader_type)?;

                let mut compiled_bytes = Vec::new();
                compiled_file.read_to_end(&mut compiled_bytes)?;

                let out_path = format!("{}/{}.spv",
                                       SPIRV_PATH,
                                       in_path.file_name().unwrap().to_string_lossy());

                std::fs::write(&out_path, &compiled_bytes)?;
            }

        }
    }
    Ok(())
}
