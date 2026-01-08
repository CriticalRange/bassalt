package com.criticalrange.bassalt.backend;

import com.mojang.blaze3d.shaders.GpuDebugOptions;
import com.mojang.blaze3d.shaders.ShaderSource;
import com.mojang.blaze3d.systems.BackendCreationException;
import com.mojang.blaze3d.systems.GpuBackend;
import com.mojang.blaze3d.systems.WindowAndDevice;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.lwjgl.glfw.GLFW;

import java.io.File;
import java.io.InputStream;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Bassalt Renderer Backend - WebGPU-based implementation for Minecraft Fabric
 *
 * This backend uses a native Rust library (wgpu-core) to provide
 * hardware-accelerated rendering through WebGPU APIs (Vulkan, DX12, Metal).
 */
@Environment(EnvType.CLIENT)
public class BassaltBackend implements GpuBackend {

    private static final Logger LOGGER = LogManager.getLogger("Bassalt");

    static {
        boolean loaded = false;
        UnsatisfiedLinkError firstError = null;

        try {
            // Try loading from library path first (development)
            System.loadLibrary("bassalt-native");
            loaded = true;
            LOGGER.debug("Native library loaded from library path");
        } catch (UnsatisfiedLinkError e1) {
            firstError = e1;
            LOGGER.debug("Library path load failed: {}", e1.getMessage());
        }

        if (!loaded) {
            try {
                // Try loading from META-INF/native/resources (packaged JAR)
                String libName = System.mapLibraryName("bassalt-native");
                LOGGER.debug("Looking for library: {}", libName);

                // Try multiple possible locations
                String[] resourcePaths = {
                        "/META-INF/native/" + libName,
                        "/native/" + libName
                };

                for (String resourcePath : resourcePaths) {
                    try (InputStream in = BassaltBackend.class.getResourceAsStream(resourcePath)) {
                        if (in != null) {
                            LOGGER.debug("Found library at: {}", resourcePath);
                            // Extract and load from temp file
                            File temp = File.createTempFile(libName, ".tmp");
                            temp.deleteOnExit();
                            java.nio.file.Files.copy(in, temp.toPath(),
                                    java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                            System.load(temp.getAbsolutePath());
                            loaded = true;
                            LOGGER.debug("Native library loaded from: {}", resourcePath);
                            break;
                        }
                    }
                }

                // Development fallback: try loading from build output directory
                if (!loaded) {
                    String cwd = System.getProperty("user.dir");
                    LOGGER.debug("Current working directory: {}", cwd);

                    // Rust replaces hyphens with underscores in library names
                    String libNameUnderscore = libName.replace("-native", "_native");
                    LOGGER.debug("Also trying with underscore: {}", libNameUnderscore);

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
                        LOGGER.debug("Checking path: {} exists: {}", libFile.getAbsolutePath(), libFile.exists());
                        if (libFile.exists()) {
                            LOGGER.debug("Found library at dev path: {}", libFile.getAbsolutePath());
                            System.load(libFile.getAbsolutePath());
                            loaded = true;
                            LOGGER.debug("Native library loaded from: {}", devPath);
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

    private static native long createDevice(long contextPtr, long windowPtr, long displayPtr, int width, int height);

    private static native String getAdapterInfo(long contextPtr);

    private final long contextPtr;

    /**
     * Create a new Bassalt backend instance
     */
    public BassaltBackend() {
        // Initialize native logging bridge first
        BassaltLogger.initNativeLogger();

        // Test the logging bridge
        LoggingTest.testLogging();

        this.contextPtr = init();
        if (this.contextPtr == 0) {
            throw new RuntimeException("Failed to initialize Basalt renderer");
        }
        LOGGER.debug("Backend initialized successfully (contextPtr: {})", contextPtr);
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
            GpuDebugOptions debugOptions) throws BackendCreationException {
        LOGGER.debug("createDeviceWithWindow CALLED: {}x{}, title={}", width, height, title);

        // Create a GLFW window without an OpenGL context
        GLFW.glfwDefaultWindowHints();
        GLFW.glfwWindowHint(GLFW.GLFW_CLIENT_API, GLFW.GLFW_NO_API);
        GLFW.glfwWindowHint(GLFW.GLFW_VISIBLE, GLFW.GLFW_TRUE);
        GLFW.glfwWindowHint(GLFW.GLFW_RESIZABLE, GLFW.GLFW_TRUE);

        long window = GLFW.glfwCreateWindow(width, height, title, monitor, 0);
        if (window == 0) {
            throw new BackendCreationException("Failed to create GLFW window");
        }

        // Get the native display and window handles from GLFW
        long displayPtr = 0;
        long nativeWindowPtr = window; // Default to GLFW window pointer

        // Detect platform and get native handles
        String osName = System.getProperty("os.name").toLowerCase();

        if (osName.contains("mac")) {
            // macOS: Get the NSView pointer for Metal
            try {
                nativeWindowPtr = org.lwjgl.glfw.GLFWNativeCocoa.glfwGetCocoaWindow(window);
                LOGGER.debug("Using macOS Cocoa - window: {}", nativeWindowPtr);
            } catch (Throwable e) {
                LOGGER.warn("Could not get macOS Cocoa window handle: {}", e.getMessage());
                nativeWindowPtr = window;
            }
        } else {
            // Linux: Try X11 first, then Wayland
            try {
                // Try X11 first (more common with XWayland)
                displayPtr = org.lwjgl.glfw.GLFWNativeX11.glfwGetX11Display();
                nativeWindowPtr = org.lwjgl.glfw.GLFWNativeX11.glfwGetX11Window(window);
                LOGGER.debug("Using X11 - display: {}, window: {}", displayPtr, nativeWindowPtr);
            } catch (Throwable e) {
                try {
                    // Fall back to Wayland if X11 fails
                    displayPtr = org.lwjgl.glfw.GLFWNativeWayland.glfwGetWaylandDisplay();
                    nativeWindowPtr = org.lwjgl.glfw.GLFWNativeWayland.glfwGetWaylandWindow(window);
                    LOGGER.debug("Using Wayland - display: {}, surface: {}", displayPtr, nativeWindowPtr);
                } catch (Throwable e2) {
                    LOGGER.warn("Could not get native display handle. X11 error: {}", e.getMessage());
                    LOGGER.debug("Wayland error: {}", e2.getMessage());
                    displayPtr = 0;
                    nativeWindowPtr = window;
                }
            }
        }

        LOGGER.debug("Calling native createDevice: contextPtr={}, window={}, display={}, size={}x{}",
                contextPtr, nativeWindowPtr, displayPtr, width, height);

        long devicePtr = createDevice(contextPtr, nativeWindowPtr, displayPtr, width, height);

        LOGGER.debug("createDevice returned: {}", devicePtr);

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
