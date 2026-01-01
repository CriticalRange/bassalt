package com.criticalrange.bassalt.backend;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.pipeline.CompiledRenderPipeline;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.shaders.ShaderSource;
import com.mojang.blaze3d.systems.CommandEncoder;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.textures.*;
import com.criticalrange.bassalt.pipeline.BassaltCommandEncoder;
import com.criticalrange.bassalt.pipeline.BassaltCompiledRenderPipeline;
import com.criticalrange.bassalt.buffer.BassaltBuffer;
import com.criticalrange.bassalt.texture.BassaltSampler;
import com.criticalrange.bassalt.texture.BassaltTexture;
import com.criticalrange.bassalt.texture.BassaltTextureView;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.jspecify.annotations.Nullable;

import java.nio.ByteBuffer;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.OptionalDouble;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

/**
 * Bassalt GPU Device - Implements Minecraft's GpuDevice interface using WebGPU
 */
@Environment(EnvType.CLIENT)
public class BassaltDevice implements GpuDevice {

    // Native method declarations
    private static native String getImplementationInfo(long ptr);
    private static native String getVendor(long ptr);
    private static native String getRenderer(long ptr);
    private static native String getVersion(long ptr);
    private static native int getMaxTextureSize(long ptr);
    private static native int getUniformOffsetAlignment(long ptr);
    private static native boolean isZZeroToOne(long ptr);
    private static native void setVsync(long ptr, boolean enabled);
    private static native void presentFrame(long ptr);
    private static native void close(long ptr);

    // Buffer operations
    private static native long createBufferEmpty(long ptr, long size, int usage);
    private static native long createBufferData(long ptr, byte[] data, int usage);
    public static native void writeBuffer(long ptr, long bufferPtr, byte[] data, long offset);
    private static native void destroyBuffer(long ptr, long bufferPtr);

    // Texture operations
    private static native long createTexture(long ptr, int width, int height, int depth,
                                              int mipLevels, int format, int usage);
    private static native long createTextureView(long ptr, long texturePtr);
    private static native void destroyTexture(long ptr, long texturePtr);

    // Sampler operations
    private static native long createSampler(long ptr, int addressModeU, int addressModeV, int addressModeW,
                                             int minFilter, int magFilter, int mipmapFilter,
                                             float lodMinClamp, float lodMaxClamp, int maxAnisotropy);

    // Pipeline operations
    private static native long createRenderPipeline(long ptr, String vertexShader, String fragmentShader,
                                                     int vertexFormat, int primitiveTopology,
                                                     boolean depthTestEnabled, boolean depthWriteEnabled,
                                                     int depthCompare, boolean blendEnabled,
                                                     int blendColorFactor, int blendAlphaFactor);

    // Create pipeline from pre-converted WGSL (for offline shader conversion)
    private static native long createNativePipelineFromWgsl(long ptr, String vertexWgsl, String fragmentWgsl,
                                                             int vertexFormat, int primitiveTopology,
                                                             boolean depthTestEnabled, boolean depthWriteEnabled,
                                                             int depthCompare, boolean blendEnabled,
                                                             int blendColorFactor, int blendAlphaFactor);

    // Render pass operations
    public static native long beginRenderPass(long ptr, long colorTexture, long depthTexture,
                                                 int clearColor, float clearDepth, int clearStencil,
                                                 int width, int height);
    public static native void setPipeline(long ptr, long renderPass, long pipeline);

    // Buffer operations - duplicate declarations removed, moved above

    private final long nativePtr;
    private final ShaderSource defaultShaderSource;
    private BassaltCommandEncoder commandEncoder;

    // Pipeline cache - maps RenderPipeline key to compiled pipeline
    private final Map<String, BassaltCompiledRenderPipeline> pipelineCache = new ConcurrentHashMap<>();

    // Cached properties
    private final String implementationInfo;
    private final int maxTextureSize;
    private final int uniformOffsetAlignment;
    private final boolean zZeroToOne;

    public BassaltDevice(long nativePtr, @Nullable ShaderSource defaultShaderSource) {
        this.nativePtr = nativePtr;
        this.defaultShaderSource = defaultShaderSource;
        this.implementationInfo = getImplementationInfo(nativePtr);
        this.maxTextureSize = getMaxTextureSize(nativePtr);
        this.uniformOffsetAlignment = getUniformOffsetAlignment(nativePtr);
        this.zZeroToOne = isZZeroToOne(nativePtr);
    }

    @Override
    public CommandEncoder createCommandEncoder() {
        if (commandEncoder == null || !commandEncoder.isValid()) {
            commandEncoder = new BassaltCommandEncoder(this);
        }
        return commandEncoder;
    }

    @Override
    public GpuSampler createSampler(
        AddressMode addressModeU,
        AddressMode addressModeV,
        FilterMode minFilter,
        FilterMode magFilter,
        int maxAnisotropy,
        OptionalDouble maxLod
    ) {
        long ptr = createSampler(
            nativePtr,
            toBassaltAddressMode(addressModeU),
            toBassaltAddressMode(addressModeV),
            toBassaltAddressMode(addressModeU),  // addressModeW - use U for now
            toBassaltFilterMode(minFilter),
            toBassaltFilterMode(magFilter),
            toBassaltFilterMode(minFilter),  // mipmapFilter - use minFilter for now
            0.0f,  // lodMinClamp
            (float)maxLod.orElse(1000.0),  // lodMaxClamp
            maxAnisotropy
        );
        return new BassaltSampler(ptr, addressModeU, addressModeV, minFilter, magFilter,
                                  maxAnisotropy, maxLod.orElse(1000.0));
    }

    @Override
    public GpuTexture createTexture(
        @Nullable Supplier<String> label,
        int usage,
        TextureFormat format,
        int width,
        int height,
        int depthOrLayers,
        int mipLevels
    ) {
        int basaltFormat = toBassaltTextureFormat(format);
        int basaltUsage = toBassaltTextureUsage(usage);

        long ptr = createTexture(nativePtr, width, height, depthOrLayers, mipLevels, basaltFormat, basaltUsage);
        String labelStr = label != null ? label.get() : "BassaltTexture";
        return new BassaltTexture(this, ptr, usage, labelStr, format, width, height, depthOrLayers, mipLevels);
    }

    @Override
    public GpuTexture createTexture(
        @Nullable String label,
        int usage,
        TextureFormat format,
        int width,
        int height,
        int depthOrLayers,
        int mipLevels
    ) {
        return createTexture(label != null ? () -> label : null, usage, format, width, height, depthOrLayers, mipLevels);
    }

    @Override
    public GpuTextureView createTextureView(GpuTexture texture) {
        BassaltTexture basaltTexture = (BassaltTexture) texture;
        long ptr = createTextureView(nativePtr, basaltTexture.getNativePtr());
        return new BassaltTextureView(texture, ptr);
    }

    @Override
    public GpuTextureView createTextureView(GpuTexture texture, int baseMipLevel, int mipLevels) {
        BassaltTexture basaltTexture = (BassaltTexture) texture;
        long ptr = createTextureView(nativePtr, basaltTexture.getNativePtr());
        return new BassaltTextureView(texture, ptr, baseMipLevel, mipLevels);
    }

    @Override
    public GpuBuffer createBuffer(@Nullable Supplier<String> label, int usage, long size) {
        long ptr = createBufferEmpty(nativePtr, size, toBassaltBufferUsage(usage));
        return new BassaltBuffer(this, ptr, usage, size);
    }

    @Override
    public GpuBuffer createBuffer(@Nullable Supplier<String> label, int usage, ByteBuffer data) {
        byte[] arr = new byte[data.remaining()];
        data.get(arr);
        long ptr = createBufferData(nativePtr, arr, toBassaltBufferUsage(usage));
        return new BassaltBuffer(this, ptr, usage, arr.length);
    }

    @Override
    public String getImplementationInformation() {
        return implementationInfo;
    }

    @Override
    public List<String> getLastDebugMessages() {
        return List.of(); // TODO: implement debug message tracking
    }

    @Override
    public boolean isDebuggingEnabled() {
        return true; // TODO: make configurable
    }

    @Override
    public String getVendor() {
        return getVendor(nativePtr);
    }

    @Override
    public String getBackendName() {
        return "Bassalt WebGPU";
    }

    @Override
    public String getVersion() {
        return getVersion(nativePtr);
    }

    @Override
    public String getRenderer() {
        return getRenderer(nativePtr);
    }

    @Override
    public int getMaxTextureSize() {
        return maxTextureSize;
    }

    @Override
    public int getUniformOffsetAlignment() {
        return uniformOffsetAlignment;
    }

    @Override
    public CompiledRenderPipeline precompilePipeline(RenderPipeline pipeline, @Nullable ShaderSource shaderSource) {
        // Create a simple cache key from the pipeline's hash code
        String cacheKey = String.valueOf(pipeline.hashCode());

        // Check cache first
        BassaltCompiledRenderPipeline cached = pipelineCache.get(cacheKey);
        if (cached != null && !cached.isClosed()) {
            return cached;
        }

        System.out.println("[Bassalt] Compiling pipeline: " + pipeline.getLocation());
        System.out.println("[Bassalt]   Vertex: " + pipeline.getVertexShader());
        System.out.println("[Bassalt]   Fragment: " + pipeline.getFragmentShader());

        // Try to load pre-converted WGSL shaders
        String vertexWgsl = loadPreconvertedWgsl(pipeline.getVertexShader(), "vert");
        String fragmentWgsl = loadPreconvertedWgsl(pipeline.getFragmentShader(), "frag");

        if (vertexWgsl == null || fragmentWgsl == null) {
            System.err.println("[Bassalt] Failed to load pre-converted WGSL shaders");
            System.err.println("[Bassalt]   Vertex WGSL: " + (vertexWgsl != null ? "loaded" : "NOT FOUND"));
            System.err.println("[Bassalt]   Fragment WGSL: " + (fragmentWgsl != null ? "loaded" : "NOT FOUND"));
            // Return invalid pipeline
            BassaltCompiledRenderPipeline compiled = new BassaltCompiledRenderPipeline(this, 0);
            pipelineCache.put(cacheKey, compiled);
            return compiled;
        }

        System.out.println("[Bassalt]   Loaded pre-converted WGSL shaders");

        // Get pipeline properties
        int vertexFormat = getVertexFormatIndex(pipeline.getVertexFormat());
        int primitiveTopology = getVertexFormatModeIndex(pipeline.getVertexFormatMode());
        boolean depthTestEnabled = pipeline.getDepthTestFunction() != com.mojang.blaze3d.platform.DepthTestFunction.NO_DEPTH_TEST;
        boolean depthWriteEnabled = pipeline.isWriteDepth();
        int depthCompare = getDepthCompareFunction(pipeline.getDepthTestFunction());
        boolean blendEnabled = pipeline.getBlendFunction().isPresent();
        int blendColorFactor = blendEnabled ? getBlendFactorIndex(pipeline.getBlendFunction().get().sourceColor()) : 0;
        int blendAlphaFactor = blendEnabled ? getBlendFactorIndex(pipeline.getBlendFunction().get().sourceAlpha()) : 0;

        // Create the native pipeline from WGSL
        long nativePipelinePtr = createNativePipelineFromWgsl(
            nativePtr,
            vertexWgsl,
            fragmentWgsl,
            vertexFormat,
            primitiveTopology,
            depthTestEnabled,
            depthWriteEnabled,
            depthCompare,
            blendEnabled,
            blendColorFactor,
            blendAlphaFactor
        );

        BassaltCompiledRenderPipeline compiled = new BassaltCompiledRenderPipeline(this, nativePipelinePtr);
        pipelineCache.put(cacheKey, compiled);

        if (nativePipelinePtr != 0) {
            System.out.println("[Bassalt]   ✓ Pipeline compiled successfully");
        } else {
            System.err.println("[Bassalt]   ✗ Pipeline compilation failed (native ptr = 0)");
        }

        return compiled;
    }

    /**
     * Load a pre-converted WGSL shader from resources
     */
    private String loadPreconvertedWgsl(net.minecraft.resources.Identifier shaderId, String stage) {
        // Convert shader ID to path: "minecraft:core/gui" -> "shaders/wgsl/core/gui.vert.wgsl" or "gui.frag.wgsl"
        String shaderPath = shaderId.getPath(); // Returns "core/gui"
        String resourcePath = "shaders/wgsl/" + shaderPath + "." + stage + ".wgsl"; // e.g., "shaders/wgsl/core/gui.vert.wgsl"

        try (var input = getClass().getResourceAsStream("/" + resourcePath)) {
            if (input == null) {
                return null;
            }
            return new String(input.readAllBytes());
        } catch (java.io.IOException e) {
            System.err.println("[Bassalt] Error loading WGSL shader " + resourcePath + ": " + e);
            return null;
        }
    }

    // Helper methods to convert Minecraft enums to Bassalt constants
    private int getVertexFormatIndex(com.mojang.blaze3d.vertex.VertexFormat format) {
        // Map vertex format to Bassalt vertex format index
        // 0 = POSITION (3 floats)
        // 1 = POSITION_COLOR (3 floats + 4 floats)
        // 2 = POSITION_TEX (3 floats + 2 floats)
        // 3 = POSITION_TEX_COLOR (3 floats + 2 floats + 4 floats)
        // 4 = POSITION_TEX_COLOR_NORMAL (3 floats + 2 floats + 4 floats + 3 floats)
        // 5 = POSITION_COLOR_TEX (3 floats + 4 floats + 2 floats)
        // 6 = POSITION_COLOR_TEX_TEX_TEX_NORMAL (position, color, uv0, uv1, uv2, normal)
        String name = format.toString().toLowerCase();

        // Parse the format string: "vertexformat[position, color, ...]"
        if (name.startsWith("vertexformat[") && name.endsWith("]")) {
            String elements = name.substring(13, name.length() - 1); // Extract "position, color, ..."
            if (elements.isEmpty()) {
                System.out.println("[Bassalt] Empty vertex format - using vertex_index mode (255)");
                return 255; // EMPTY - shader uses @builtin(vertex_index)
            }

            String[] parts = elements.split(",\\s*");

            // Count element types
            boolean hasPosition = false;
            boolean hasColor = false;
            int uvCount = 0;
            boolean hasNormal = false;

            for (String part : parts) {
                if (part.equals("position")) hasPosition = true;
                else if (part.equals("color")) hasColor = true;
                else if (part.startsWith("uv")) uvCount++;
                else if (part.equals("normal")) hasNormal = true;
            }

            // Map to format index based on elements
            if (hasPosition && !hasColor && uvCount == 0 && !hasNormal) {
                return 0; // POSITION
            } else if (hasPosition && hasColor && uvCount == 0 && !hasNormal) {
                return 1; // POSITION_COLOR
            } else if (hasPosition && !hasColor && uvCount == 1 && !hasNormal) {
                return 2; // POSITION_TEX
            } else if (hasPosition && hasColor && uvCount == 1 && !hasNormal) {
                // Check element order
                if (parts.length >= 2 && parts[1].equals("color")) {
                    return 5; // POSITION_COLOR_TEX
                } else {
                    return 3; // POSITION_TEX_COLOR
                }
            } else if (hasPosition && hasColor && uvCount == 1 && hasNormal) {
                return 4; // POSITION_TEX_COLOR_NORMAL (reusing for POSITION_COLOR_TEX_NORMAL)
            } else if (hasPosition && hasColor && uvCount == 2 && hasNormal) {
                return 7; // POSITION_COLOR_TEX_TEX_NORMAL (position, color, uv0, uv2, normal)
            } else if (hasPosition && hasColor && uvCount == 3 && hasNormal) {
                return 6; // POSITION_COLOR_TEX_TEX_TEX_NORMAL
            }
        }

        System.err.println("[Bassalt] Unknown vertex format: " + name + ", defaulting to position_tex_color");
        return 3;
    }

    private int getVertexFormatModeIndex(com.mojang.blaze3d.vertex.VertexFormat.Mode mode) {
        // Map VertexFormat.Mode to primitive topology
        return switch (mode) {
            case POINTS -> BassaltBackend.PRIMITIVE_TOPOLOGY_POINT_LIST;
            case LINES -> BassaltBackend.PRIMITIVE_TOPOLOGY_LINE_LIST;
            case TRIANGLES -> BassaltBackend.PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
            case TRIANGLE_STRIP -> BassaltBackend.PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP;
            // For quad rendering, we'll use triangle list (quad conversion happens elsewhere)
            case QUADS -> BassaltBackend.PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
            // Line strip and debug modes fall back to line list
            case DEBUG_LINE_STRIP, DEBUG_LINES -> BassaltBackend.PRIMITIVE_TOPOLOGY_LINE_LIST;
            // Triangle fan isn't directly supported, fall back to triangle list
            case TRIANGLE_FAN -> BassaltBackend.PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
        };
    }

    private int getDepthCompareFunction(com.mojang.blaze3d.platform.DepthTestFunction function) {
        return switch (function) {
            case NO_DEPTH_TEST -> BassaltBackend.COMPARE_FUNC_ALWAYS;
            case EQUAL_DEPTH_TEST -> BassaltBackend.COMPARE_FUNC_EQUAL;
            case LEQUAL_DEPTH_TEST -> BassaltBackend.COMPARE_FUNC_LESS_EQUAL;
            case LESS_DEPTH_TEST -> BassaltBackend.COMPARE_FUNC_LESS;
            case GREATER_DEPTH_TEST -> BassaltBackend.COMPARE_FUNC_GREATER;
        };
    }

    private int getBlendFactorIndex(com.mojang.blaze3d.platform.SourceFactor factor) {
        return switch (factor) {
            case ZERO -> BassaltBackend.BLEND_FACTOR_ZERO;
            case ONE -> BassaltBackend.BLEND_FACTOR_ONE;
            case SRC_COLOR -> BassaltBackend.BLEND_FACTOR_SRC;
            case ONE_MINUS_SRC_COLOR -> BassaltBackend.BLEND_FACTOR_ONE_MINUS_SRC;
            case DST_COLOR -> BassaltBackend.BLEND_FACTOR_DST;
            case ONE_MINUS_DST_COLOR -> BassaltBackend.BLEND_FACTOR_ONE_MINUS_DST;
            case SRC_ALPHA -> BassaltBackend.BLEND_FACTOR_SRC_ALPHA;
            case ONE_MINUS_SRC_ALPHA -> BassaltBackend.BLEND_FACTOR_ONE_MINUS_SRC_ALPHA;
            case DST_ALPHA -> BassaltBackend.BLEND_FACTOR_DST_ALPHA;
            case ONE_MINUS_DST_ALPHA -> BassaltBackend.BLEND_FACTOR_ONE_MINUS_DST_ALPHA;
            case CONSTANT_COLOR, CONSTANT_ALPHA, ONE_MINUS_CONSTANT_COLOR, ONE_MINUS_CONSTANT_ALPHA, SRC_ALPHA_SATURATE ->
                BassaltBackend.BLEND_FACTOR_ONE; // Fallback for less common factors
        };
    }

    @Override
    public void clearPipelineCache() {
        // Close all cached pipelines
        for (BassaltCompiledRenderPipeline pipeline : pipelineCache.values()) {
            pipeline.close();
        }
        pipelineCache.clear();
    }

    @Override
    public List<String> getEnabledExtensions() {
        String features = getEnabledFeatures0(nativePtr);
        if (features == null || features.isEmpty()) {
            return List.of();
        }
        return Arrays.asList(features.split(", "));
    }

    @Override
    public int getMaxSupportedAnisotropy() {
        return getMaxSupportedAnisotropy0(nativePtr);
    }

    private static native int getMaxSupportedAnisotropy0(long ptr);
    private static native String getEnabledFeatures0(long ptr);

    @Override
    public void close() {
        // Clean up pipeline cache
        clearPipelineCache();
        // Close native device
        close(nativePtr);
    }

    @Override
    public void setVsync(boolean enabled) {
        setVsync(nativePtr, enabled);
    }

    @Override
    public void presentFrame() {
        presentFrame(nativePtr);
    }

    @Override
    public boolean isZZeroToOne() {
        return zZeroToOne;
    }

    public long getNativePtr() {
        return nativePtr;
    }

    @Nullable
    public ShaderSource getDefaultShaderSource() {
        return defaultShaderSource;
    }

    // Type conversion helpers

    private static int toBassaltAddressMode(AddressMode mode) {
        // AddressMode in MC 26.1 only has REPEAT and CLAMP_TO_EDGE
        return switch (mode) {
            case REPEAT -> BassaltBackend.ADDRESS_MODE_REPEAT;
            case CLAMP_TO_EDGE -> BassaltBackend.ADDRESS_MODE_CLAMP_TO_EDGE;
        };
    }

    private static int toBassaltFilterMode(FilterMode mode) {
        return switch (mode) {
            case NEAREST -> BassaltBackend.FILTER_MODE_NEAREST;
            case LINEAR -> BassaltBackend.FILTER_MODE_LINEAR;
        };
    }

    private static int toBassaltBufferUsage(int minecraftUsage) {
        int usage = 0;
        // Minecraft's GpuBuffer.Usage flags:
        // MAP_READ=1, MAP_WRITE=2, CLIENT_STORAGE=4, COPY_DST=8, COPY_SRC=16,
        // VERTEX=32, INDEX=64, UNIFORM=128, UNIFORM_TEXEL_BUFFER=256
        
        // MAP_READ/MAP_WRITE (0x01, 0x02) - WebGPU handles mapping differently, skip for now
        if ((minecraftUsage & 0x08) != 0) usage |= BassaltBackend.BUFFER_USAGE_COPY_DST;  // COPY_DST
        if ((minecraftUsage & 0x10) != 0) usage |= BassaltBackend.BUFFER_USAGE_COPY_SRC;  // COPY_SRC
        if ((minecraftUsage & 0x20) != 0) usage |= BassaltBackend.BUFFER_USAGE_VERTEX;   // VERTEX
        if ((minecraftUsage & 0x40) != 0) usage |= BassaltBackend.BUFFER_USAGE_INDEX;    // INDEX
        if ((minecraftUsage & 0x80) != 0) usage |= BassaltBackend.BUFFER_USAGE_UNIFORM;  // UNIFORM

        // WebGPU requires COPY_DST to upload buffer data, but OpenGL doesn't distinguish.
        // Always add COPY_DST so we can write to any buffer (like OpenGL).
        usage |= BassaltBackend.BUFFER_USAGE_COPY_DST;

        return usage;
    }

    private static int toBassaltTextureFormat(TextureFormat format) {
        // TextureFormat in MC 26.1 only has: RGBA8, RED8, RED8I, DEPTH32
        return switch (format) {
            case RGBA8 -> BassaltBackend.FORMAT_RGBA8;
            case RED8, RED8I -> BassaltBackend.FORMAT_R8;
            case DEPTH32 -> BassaltBackend.FORMAT_DEPTH32F;
        };
    }

    private static int toBassaltTextureUsage(int minecraftUsage) {
        int usage = 0;
        // Map Minecraft usage flags to Bassalt usage flags
        // NOTE: Minecraft and Bassalt use different bit positions for the same flags!
        // Minecraft: COPY_DST=1, COPY_SRC=2, TEXTURE_BINDING=4, RENDER_ATTACHMENT=8, CUBEMAP=16
        // Bassalt:   COPY_SRC=1, COPY_DST=2, TEXTURE_BINDING=4, STORAGE=8, RENDER_ATTACHMENT=16
        if ((minecraftUsage & 0x01) != 0) usage |= BassaltBackend.TEXTURE_USAGE_COPY_DST;  // MC COPY_DST → Bassalt COPY_DST
        if ((minecraftUsage & 0x02) != 0) usage |= BassaltBackend.TEXTURE_USAGE_COPY_SRC;  // MC COPY_SRC → Bassalt COPY_SRC
        if ((minecraftUsage & 0x04) != 0) usage |= BassaltBackend.TEXTURE_USAGE_TEXTURE_BINDING;  // MC TEXTURE_BINDING → Bassalt TEXTURE_BINDING
        if ((minecraftUsage & 0x08) != 0) usage |= BassaltBackend.TEXTURE_USAGE_RENDER_ATTACHMENT;  // MC RENDER_ATTACHMENT → Bassalt RENDER_ATTACHMENT
        // Note: Minecraft's CUBEMAP_COMPATIBLE (0x10) doesn't have a direct Bassalt equivalent

        // WebGPU requires COPY_DST to upload texture data, but OpenGL doesn't distinguish.
        // Always add COPY_DST so we can write to any texture (like OpenGL).
        // This is safe and matches OpenGL's "any texture can be uploaded to" behavior.
        usage |= BassaltBackend.TEXTURE_USAGE_COPY_DST;

        return usage;
    }

    // Package-private methods for internal use

    void createNativeBuffer(long bufferPtr, long size, int usage) {
        // Helper for creating buffer wrappers
    }

    public void destroyNativeBuffer(long bufferPtr) {
        destroyBuffer(nativePtr, bufferPtr);
    }

    public void destroyNativeTexture(long texturePtr) {
        destroyTexture(nativePtr, texturePtr);
    }

    public long createNativePipeline(String vertexWgsl, String fragmentWgsl,
                               int vertexFormat, int primitiveTopology,
                               boolean depthTestEnabled, boolean depthWriteEnabled,
                               int depthCompare, boolean blendEnabled,
                               int blendColorFactor, int blendAlphaFactor) {
        return createRenderPipeline(nativePtr, vertexWgsl, fragmentWgsl,
                vertexFormat, primitiveTopology, depthTestEnabled, depthWriteEnabled,
                depthCompare, blendEnabled, blendColorFactor, blendAlphaFactor);
    }

    // Public access to native render pass methods for BassaltRenderPass
    public static native void setVertexBuffer(long ptr, long renderPass, int slot, long buffer, long offset);
    public static native void setIndexBuffer(long ptr, long renderPass, long buffer, int indexType, long offset);
    public static native void drawIndexed(long ptr, long renderPass, int indexCount, int instanceCount, int firstIndex, int baseVertex, int firstInstance);
    public static native void draw(long ptr, long renderPass, int vertexCount, int instanceCount, int firstVertex, int firstInstance);
    public static native void setScissorRect(long ptr, long renderPass, int x, int y, int width, int height);
    public static native void endRenderPass(long ptr, long renderPass);
}
