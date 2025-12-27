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
    private static native long createSampler(long ptr, int addressModeU, int addressModeV,
                                             int minFilter, int magFilter, int maxAnisotropy);

    // Pipeline operations
    private static native long createRenderPipeline(long ptr, String vertexShader, String fragmentShader,
                                                     int vertexFormat, int primitiveTopology,
                                                     boolean depthTestEnabled, boolean depthWriteEnabled,
                                                     int depthCompare, boolean blendEnabled,
                                                     int blendColorFactor, int blendAlphaFactor);

    // Render pass operations
    public static native long beginRenderPass(long ptr, long colorTexture, long depthTexture,
                                                 int clearColor, float clearDepth, int clearStencil,
                                                 int width, int height);
    private static native void setPipeline(long ptr, long renderPass, long pipeline);

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
            toBassaltFilterMode(minFilter),
            toBassaltFilterMode(magFilter),
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
        return new BassaltTexture(this, ptr, format, width, height);
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
        return false; // TODO: make configurable
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

        // For now, create a placeholder pipeline
        // TODO: Implement full pipeline compilation with:
        // 1. Getting shaders from shaderSource or pipeline
        // 2. Translating GLSL to WGSL
        // 3. Creating the WebGPU render pipeline
        BassaltCompiledRenderPipeline compiled = new BassaltCompiledRenderPipeline(this, 0);
        pipelineCache.put(cacheKey, compiled);

        return compiled;
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
        return List.of(); // TODO: query from wgpu
    }

    @Override
    public int getMaxSupportedAnisotropy() {
        return 16; // TODO: query from device
    }

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
        // Map Minecraft's GpuBuffer.Usage to Bassalt's flags
        if ((minecraftUsage & 0x01) != 0) usage |= BassaltBackend.BUFFER_USAGE_COPY_SRC;
        if ((minecraftUsage & 0x02) != 0) usage |= BassaltBackend.BUFFER_USAGE_COPY_DST;
        if ((minecraftUsage & 0x20) != 0) usage |= BassaltBackend.BUFFER_USAGE_VERTEX;
        if ((minecraftUsage & 0x40) != 0) usage |= BassaltBackend.BUFFER_USAGE_INDEX;
        if ((minecraftUsage & 0x80) != 0) usage |= BassaltBackend.BUFFER_USAGE_UNIFORM;
        if ((minecraftUsage & 0x200) != 0) usage |= BassaltBackend.BUFFER_USAGE_STORAGE;
        if ((minecraftUsage & 0x08) != 0) usage |= BassaltBackend.BUFFER_USAGE_INDIRECT;
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
        if ((minecraftUsage & 0x01) != 0) usage |= BassaltBackend.TEXTURE_USAGE_COPY_SRC;
        if ((minecraftUsage & 0x02) != 0) usage |= BassaltBackend.TEXTURE_USAGE_COPY_DST;
        if ((minecraftUsage & 0x04) != 0) usage |= BassaltBackend.TEXTURE_USAGE_TEXTURE_BINDING;
        if ((minecraftUsage & 0x08) != 0) usage |= BassaltBackend.TEXTURE_USAGE_STORAGE_BINDING;
        if ((minecraftUsage & 0x10) != 0) usage |= BassaltBackend.TEXTURE_USAGE_RENDER_ATTACHMENT;
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
    public static native void endRenderPass(long ptr, long renderPass);
}
