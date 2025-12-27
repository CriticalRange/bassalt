package com.criticalrange.bassalt.shader;

import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

/**
 * WGSL Shader Compiler - Translates GLSL shaders to WGSL using naga
 */
@Environment(EnvType.CLIENT)
public class WgslCompiler {

    static {
        // Native library is loaded by BassaltBackend
    }

    // Shader stage constants
    private static final int STAGE_VERTEX = 0;
    private static final int STAGE_FRAGMENT = 1;
    private static final int STAGE_COMPUTE = 2;

    // Native method declaration
    private static native String translateGlslToWgsl(String glslSource, int stage);

    /**
     * Translate a vertex shader from GLSL to WGSL
     */
    public static String translateVertexShader(String glslSource) {
        return translateGlslToWgsl(glslSource, STAGE_VERTEX);
    }

    /**
     * Translate a fragment shader from GLSL to WGSL
     */
    public static String translateFragmentShader(String glslSource) {
        return translateGlslToWgsl(glslSource, STAGE_FRAGMENT);
    }

    /**
     * Translate a compute shader from GLSL to WGSL
     */
    public static String translateComputeShader(String glslSource) {
        return translateGlslToWgsl(glslSource, STAGE_COMPUTE);
    }

    /**
     * Translate GLSL shader to WGSL
     *
     * @param glslSource GLSL shader source code
     * @param stage Shader stage (0=vertex, 1=fragment, 2=compute)
     * @return WGSL shader source code
     */
    public static String translate(String glslSource, int stage) {
        return translateGlslToWgsl(glslSource, stage);
    }

    /**
     * Preprocess Minecraft's GLSL shader format
     * Handles moj_import directives and other Minecraft-specific syntax
     */
    public static String preprocessMinecraftShader(String glslSource) {
        String result = glslSource;

        // Remove version declaration
        result = result.replaceAll("#version\\s+\\d+\\s*(core|es)?\\n", "");

        // Handle moj_import directives - replace with actual includes
        // For now, just comment them out
        result = result.replaceAll("#moj_import\\s+<([^>]+)>", "// moj_import: $1");

        // Remove precision qualifiers (not used in WGSL)
        result = result.replaceAll("precision\\s+\\w+\\s+\\w+\\s*;", "");

        return result;
    }

    /**
     * Convert Minecraft GLSL builtins to WGSL equivalents
     */
    public static String convertBuiltins(String wgslSource, int stage) {
        String result = wgslSource;

        if (stage == STAGE_VERTEX) {
            // gl_Position -> builtin(position)
            result = result.replace("gl_Position", "builtin.position");
            result = result.replace("gl_VertexID", "builtin.vertex_index");
            result = result.replace("gl_InstanceID", "builtin.instance_index");
        } else if (stage == STAGE_FRAGMENT) {
            // gl_FragColor -> return value
            result = result.replaceAll("gl_FragColor\\s*=\\s*([^;]+);", "return $1;");
            result = result.replace("gl_FragCoord", "builtin.position");
            result = result.replace("gl_FrontFacing", "builtin.front_facing");
            result = result.replace("gl_FragDepth", "builtin.frag_depth");
        }

        return result;
    }
}
