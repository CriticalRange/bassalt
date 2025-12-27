#!/usr/bin/env python3
"""
Convert Minecraft GLSL shaders to WGSL for Bassalt/WebGPU

This script:
1. Preprocesses #moj_import directives
2. Removes #version directives
3. Converts GLSL builtins to compatible forms
4. Uses naga to convert GLSL → WGSL
"""

import os
import re
import sys
import subprocess
from pathlib import Path
from typing import Dict, Set

# Shader directory structure
SHADERS_DIR = Path(__file__).parent.parent / "src" / "main" / "resources" / "shaders"
OUTPUT_DIR = Path(__file__).parent.parent / "src" / "main" / "resources" / "shaders" / "wgsl"

# File extensions
VERTEX_EXT = ".vsh"
FRAGMENT_EXT = ".fsh"


class ShaderPreprocessor:
    """Preprocess GLSL shaders with #moj_import directives"""

    def __init__(self, include_dir: Path):
        self.include_dir = include_dir
        self.included_files: Set[str] = set()
        self.binding_counter = 0  # Global binding counter for uniform buffers

    def preprocess_moj_imports(self, source: str, current_file: Path = None) -> str:
        """Process #moj_import <> directives"""
        self.included_files = set()

        def replace_import(match):
            import_path = match.group(1)
            # Convert minecraft:filename.glsl to actual file path
            if import_path.startswith("minecraft:"):
                filename = import_path.replace("minecraft:", "")
                full_path = self.include_dir / filename

                if str(full_path) in self.included_files:
                    return f"// Already included: {import_path}\n"

                if full_path.exists():
                    self.included_files.add(str(full_path))
                    try:
                        with open(full_path, 'r') as f:
                            included_source = f.read()
                            # Fully preprocess the included file (remove #version, etc)
                            included_source = self.preprocess(included_source, full_path)
                            return f"// Import: {import_path}\n{included_source}\n"
                    except Exception as e:
                        return f"// Error importing {import_path}: {e}\n"
                else:
                    return f"// Missing import: {import_path}\n"

            return f"// Unknown import: {import_path}\n"

        # Process #moj_import directives
        pattern = r'#moj_import\s+<([^>]+)>'
        result = re.sub(pattern, replace_import, source)

        return result

    def preprocess(self, source: str, current_file: Path = None) -> str:
        """Full preprocessing pipeline"""
        # Remove #version directives (including those in included files)
        source = re.sub(r'#version\s+\d+', '', source)

        # Process #moj_import directives
        source = self.preprocess_moj_imports(source, current_file)

        # Remove precision qualifiers (not needed in WGSL)
        source = re.sub(r'precision\s+\w+\s+\w+;', '', source)

        # Add binding indices to uniform buffers - naga requires explicit bindings
        # Use instance variable to maintain counter across recursive calls
        def add_binding(match):
            binding = self.binding_counter
            self.binding_counter += 1
            return f"layout(std140, binding={binding}) uniform {match.group(1)}"

        source = re.sub(r'layout\(std140\)\s+uniform\s+(\w+)', add_binding, source)

        # Convert uniform sampler2D declarations to texture+separator combinations
        # Naga doesn't support combined image samplers in GLSL frontend
        # We'll mark them for manual handling
        source = re.sub(
            r'uniform\s+sampler2D\s+(\w+);',
            r'// SAMPLER: \1 - requires texture+sampler binding',
            source
        )

        return source


def convert_glsl_to_wgsl(glsl_source: str, shader_stage: str) -> str:
    """Convert GLSL to WGSL using naga"""

    # Write temporary GLSL file
    temp_glsl = Path("/tmp/temp_shader.glsl")
    temp_wgsl = Path("/tmp/temp_shader.wgsl")

    with open(temp_glsl, 'w') as f:
        f.write(glsl_source)

    # Use naga to convert - CLI format is: naga [input] [output...]
    try:
        stage_map = {
            "vertex": "vert",
            "fragment": "frag",
            "compute": "comp"
        }

        naga_stage = stage_map.get(shader_stage, shader_stage)

        result = subprocess.run(
            ['naga', '--input-kind', 'glsl', '--shader-stage', naga_stage,
             str(temp_glsl), str(temp_wgsl)],
            capture_output=True,
            text=True
        )

        if result.returncode == 0:
            with open(temp_wgsl, 'r') as f:
                return f.read()
        else:
            print(f"Naga error: {result.stderr}", file=sys.stderr)
            # Return the preprocessed GLSL with error comment
            return f"// Conversion failed: {result.stderr}\n{glsl_source}"

    except Exception as e:
        print(f"Naga execution error: {e}", file=sys.stderr)
        # Return the preprocessed GLSL with error comment
        return f"// Conversion error: {e}\n{glsl_source}"


def process_shader(input_file: Path, output_dir: Path, preprocessor: ShaderPreprocessor):
    """Process a single shader file"""
    print(f"Processing {input_file.name}...")

    # Read source
    with open(input_file, 'r') as f:
        source = f.read()

    # Determine shader stage
    if input_file.suffix == VERTEX_EXT:
        stage = "vertex"
    elif input_file.suffix == FRAGMENT_EXT:
        stage = "fragment"
    else:
        print(f"  Skipping unknown shader type: {input_file.suffix}")
        return

    # Preprocess
    preprocessed = preprocessor.preprocess(source, input_file)

    # Convert to WGSL
    try:
        wgsl = convert_glsl_to_wgsl(preprocessed, stage)

        # Write output with stage suffix to avoid collisions
        stage_suffix = "vert" if stage == "vertex" else "frag"
        output_file = output_dir / f"{input_file.stem}.{stage_suffix}.wgsl"
        output_file.parent.mkdir(parents=True, exist_ok=True)

        with open(output_file, 'w') as f:
            f.write(wgsl)

        print(f"  ✓ Converted to {output_file.name}")

    except Exception as e:
        print(f"  ✗ Failed: {e}")


def main():
    """Main conversion entry point"""
    print("Converting Minecraft GLSL shaders to WGSL...")

    include_dir = SHADERS_DIR / "include"
    core_dir = SHADERS_DIR / "core"
    output_dir = OUTPUT_DIR

    # Create preprocessor
    preprocessor = ShaderPreprocessor(include_dir)

    # Process all core shaders
    if core_dir.exists():
        for shader_file in core_dir.glob("*.vsh"):
            process_shader(shader_file, output_dir / "core", preprocessor)

        for shader_file in core_dir.glob("*.fsh"):
            process_shader(shader_file, output_dir / "core", preprocessor)

    print("\nConversion complete!")
    print(f"WGSL shaders written to: {output_dir}")


if __name__ == "__main__":
    main()
