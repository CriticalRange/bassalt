package com.criticalrange.bassalt.backend;

import com.mojang.blaze3d.opengl.GlBackend;
import com.mojang.blaze3d.shaders.GpuDebugOptions;
import com.mojang.blaze3d.shaders.ShaderSource;
import com.mojang.blaze3d.systems.BackendCreationException;
import com.mojang.blaze3d.systems.WindowAndDevice;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.lwjgl.glfw.GLFW;

import java.io.File;
import java.io.InputStream;

/**
 * Bassalt Renderer Backend - WebGPU-based implementation for Minecraft Fabric
 *
 * This backend uses a native Rust library (wgpu-core) to provide hardware-accelerated
 * rendering through WebGPU APIs (Vulkan, DX12, Metal).
 *
 * Note: Extends GlBackend to satisfy Mixin @Redirect type requirements, but completely
 * overrides all behavior with wgpu-based implementation.
 */
@Environment(EnvType.CLIENT)
public class BassaltBackend extends GlBackend {

    static {
        boolean loaded = false;
        UnsatisfiedLinkError firstError = null;

        try {
            // Try loading from library path first (development)
            System.loadLibrary("bassalt-native");
            loaded = true;
            System.out.println("[Bassalt] Native library loaded from library path");
        } catch (UnsatisfiedLinkError e1) {
            firstError = e1;
            System.out.println("[Bassalt] Library path load failed: " + e1.getMessage());
        }

        if (!loaded) {
            try {
                // Try loading from META-INF/native/resources (packaged JAR)
                String libName = System.mapLibraryName("bassalt-native");
                System.out.println("[Bassalt] Looking for library: " + libName);

                // Try multiple possible locations
                String[] resourcePaths = {
                    "/META-INF/native/" + libName,
                    "/native/" + libName
                };

                for (String resourcePath : resourcePaths) {
                    try (InputStream in = BassaltBackend.class.getResourceAsStream(resourcePath)) {
                        if (in != null) {
                            System.out.println("[Bassalt] Found library at: " + resourcePath);
                            // Extract and load from temp file
                            File temp = File.createTempFile(libName, ".tmp");
                            temp.deleteOnExit();
                            java.nio.file.Files.copy(in, temp.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                            System.load(temp.getAbsolutePath());
                            loaded = true;
                            System.out.println("[Bassalt] Native library loaded from: " + resourcePath);
                            break;
                        }
                    }
                }

                // Development fallback: try loading from build output directory
                if (!loaded) {
                    String cwd = System.getProperty("user.dir");
                    System.out.println("[Bassalt] Current working directory: " + cwd);

                    // Rust replaces hyphens with underscores in library names
                    String libNameUnderscore = libName.replace("-native", "_native");
                    System.out.println("[Bassalt] Also trying with underscore: " + libNameUnderscore);

                    String[] devPaths = {
                        "bassalt-native/target/release/" + libName,
                        "bassalt-native/target/release/" + libNameUnderscore,
                        "../bassalt-native/target/release/" + libName,
                        "../bassalt-native/target/release/" + libNameUnderscore,
                        "../../bassalt-native/target/release/" + libName,
                        "../../bassalt-native/target/release/" + libNameUnderscore
                    };

                    for (String devPath : devPaths) {
                        File libFile = new File(devPath);
                        System.out.println("[Bassalt] Checking path: " + libFile.getAbsolutePath() + " exists: " + libFile.exists());
                        if (libFile.exists()) {
                            System.out.println("[Bassalt] Found library at dev path: " + libFile.getAbsolutePath());
                            System.load(libFile.getAbsolutePath());
                            loaded = true;
                            System.out.println("[Bassalt] Native library loaded from: " + devPath);
                            break;
                        }
                    }
                }

                if (!loaded) {
                    throw new RuntimeException("Bassalt native library not found in any resource path", firstError);
                }
            } catch (java.io.IOException e2) {
                throw new RuntimeException("Failed to load Bassalt native library", e2);
            }
        }
    }

    /**
     * Constants for buffer usage flags - match Rust definitions
     */
    public static final int BUFFER_USAGE_COPY_SRC = 1 << 0;
    public static final int BUFFER_USAGE_COPY_DST = 1 << 1;
    public static final int BUFFER_USAGE_VERTEX = 1 << 2;
    public static final int BUFFER_USAGE_INDEX = 1 << 3;
    public static final int BUFFER_USAGE_UNIFORM = 1 << 4;
    public static final int BUFFER_USAGE_STORAGE = 1 << 5;
    public static final int BUFFER_USAGE_INDIRECT = 1 << 6;

    /**
     * Constants for texture usage flags
     */
    public static final int TEXTURE_USAGE_COPY_SRC = 1 << 0;
    public static final int TEXTURE_USAGE_COPY_DST = 1 << 1;
    public static final int TEXTURE_USAGE_TEXTURE_BINDING = 1 << 2;
    public static final int TEXTURE_USAGE_STORAGE_BINDING = 1 << 3;
    public static final int TEXTURE_USAGE_RENDER_ATTACHMENT = 1 << 4;

    /**
     * Texture format constants
     */
    public static final int FORMAT_RGBA8 = 0;
    public static final int FORMAT_BGRA8 = 1;
    public static final int FORMAT_RGB8 = 2;
    public static final int FORMAT_RG8 = 3;
    public static final int FORMAT_R8 = 4;
    public static final int FORMAT_RGBA16F = 5;
    public static final int FORMAT_RGBA32F = 6;
    public static final int FORMAT_DEPTH24 = 7;
    public static final int FORMAT_DEPTH32F = 8;
    public static final int FORMAT_DEPTH24_STENCIL8 = 9;

    /**
     * Address mode constants
     */
    public static final int ADDRESS_MODE_REPEAT = 0;
    public static final int ADDRESS_MODE_MIRRORED_REPEAT = 1;
    public static final int ADDRESS_MODE_CLAMP_TO_EDGE = 2;
    public static final int ADDRESS_MODE_CLAMP_TO_BORDER = 3;

    /**
     * Filter mode constants
     */
    public static final int FILTER_MODE_NEAREST = 0;
    public static final int FILTER_MODE_LINEAR = 1;

    /**
     * Blend factor constants
     */
    public static final int BLEND_FACTOR_ZERO = 0;
    public static final int BLEND_FACTOR_ONE = 1;
    public static final int BLEND_FACTOR_SRC = 2;
    public static final int BLEND_FACTOR_ONE_MINUS_SRC = 3;
    public static final int BLEND_FACTOR_DST = 4;
    public static final int BLEND_FACTOR_ONE_MINUS_DST = 5;
    public static final int BLEND_FACTOR_SRC_ALPHA = 6;
    public static final int BLEND_FACTOR_ONE_MINUS_SRC_ALPHA = 7;
    public static final int BLEND_FACTOR_DST_ALPHA = 8;
    public static final int BLEND_FACTOR_ONE_MINUS_DST_ALPHA = 9;

    /**
     * Compare function constants
     */
    public static final int COMPARE_FUNC_NEVER = 0;
    public static final int COMPARE_FUNC_LESS = 1;
    public static final int COMPARE_FUNC_EQUAL = 2;
    public static final int COMPARE_FUNC_LESS_EQUAL = 3;
    public static final int COMPARE_FUNC_GREATER = 4;
    public static final int COMPARE_FUNC_NOT_EQUAL = 5;
    public static final int COMPARE_FUNC_GREATER_EQUAL = 6;
    public static final int COMPARE_FUNC_ALWAYS = 7;

    /**
     * Primitive topology constants
     */
    public static final int PRIMITIVE_TOPOLOGY_POINT_LIST = 0;
    public static final int PRIMITIVE_TOPOLOGY_LINE_LIST = 1;
    public static final int PRIMITIVE_TOPOLOGY_LINE_STRIP = 2;
    public static final int PRIMITIVE_TOPOLOGY_TRIANGLE_LIST = 3;
    public static final int PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP = 4;

    // Native method declarations
    private static native long init();
    private static native long createDevice(long contextPtr, long windowPtr, int width, int height);
    private static native String getAdapterInfo(long contextPtr);

    private final long contextPtr;

    /**
     * Create a new Bassalt backend instance
     */
    public BassaltBackend() {
        this.contextPtr = init();
        if (this.contextPtr == 0) {
            throw new RuntimeException("Failed to initialize Bassalt renderer");
        }
        // TODO: getAdapterInfo is temporarily disabled due to JNI linking issue
        // System.out.println("[Bassalt] Backend initialized: " + getAdapterInfo(contextPtr));
        System.out.println("[Bassalt] Backend initialized successfully (contextPtr: " + contextPtr + ")");
    }

    @Override
    public String getName() {
        return "Bassalt (WebGPU)";
    }

    @Override
    public WindowAndDevice createDeviceWithWindow(
        int width,
        int height,
        String title,
        long monitor,
        ShaderSource defaultShaderSource,
        GpuDebugOptions debugOptions
    ) throws BackendCreationException {
        // Create a GLFW window without an OpenGL context
        GLFW.glfwDefaultWindowHints();
        GLFW.glfwWindowHint(GLFW.GLFW_CLIENT_API, GLFW.GLFW_NO_API);
        GLFW.glfwWindowHint(GLFW.GLFW_VISIBLE, GLFW.GLFW_TRUE);
        GLFW.glfwWindowHint(GLFW.GLFW_RESIZABLE, GLFW.GLFW_TRUE);

        long window = GLFW.glfwCreateWindow(width, height, title, monitor, 0);
        if (window == 0) {
            throw new BackendCreationException("Failed to create GLFW window");
        }

        // Create the device
        long devicePtr = createDevice(contextPtr, window, width, height);
        if (devicePtr == 0) {
            GLFW.glfwDestroyWindow(window);
            throw new BackendCreationException("Failed to create Bassalt device");
        }

        BassaltDevice device = new BassaltDevice(devicePtr, defaultShaderSource);
        return new WindowAndDevice(window, device);
    }

    @Override
    public String toString() {
        return getName();
    }
}
