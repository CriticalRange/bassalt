package com.criticalrange.bassalt.pipeline;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.criticalrange.bassalt.texture.BassaltTexture;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.buffers.GpuFence;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.systems.*;
import com.mojang.blaze3d.textures.GpuSampler;
import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.VertexFormat;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.jspecify.annotations.Nullable;

import java.nio.ByteBuffer;
import java.util.Collection;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.OptionalLong;
import java.util.function.BiConsumer;
import java.util.function.Supplier;

/**
 * Bassalt Command Encoder - Implements Minecraft's CommandEncoder interface
 */
@Environment(EnvType.CLIENT)
public class BassaltCommandEncoder implements CommandEncoder {

    private final BassaltDevice device;
    private long currentRenderPass = 0;
    private boolean isActive = false;
    private boolean finished = false;

    // Native method declarations
    private static native void writeToTexture0(long devicePtr, long texturePtr, byte[] data,
                                                int mipLevel, int depthOrLayer, int destX, int destY,
                                                int width, int height, int format);
    private static native void copyToBuffer0(long devicePtr, long srcBufferPtr, long dstBufferPtr,
                                              long srcOffset, long dstOffset, long size);
    private static native void copyTextureToBuffer0(long devicePtr, long texturePtr, long bufferPtr,
                                                     long bufferOffset, int mipLevel, int width, int height);

    public BassaltCommandEncoder(BassaltDevice device) {
        this.device = device;
    }

    public boolean isValid() {
        return !finished;
    }

    @Override
    public RenderPass createRenderPass(
        @Nullable Supplier<String> label,
        @Nullable GpuTextureView colorTexture,
        OptionalInt clearColor
    ) {
        return createRenderPass(label, colorTexture, clearColor, null, OptionalDouble.empty());
    }

    @Override
    public RenderPass createRenderPass(
        @Nullable Supplier<String> label,
        @Nullable GpuTextureView colorTexture,
        OptionalInt clearColor,
        @Nullable GpuTextureView depthTexture,
        OptionalDouble clearDepth
    ) {
        // Get native pointers from texture views
        long colorPtr = 0;
        long depthPtr = 0;

        if (colorTexture instanceof com.criticalrange.bassalt.texture.BassaltTextureView) {
            colorPtr = ((com.criticalrange.bassalt.texture.BassaltTextureView) colorTexture).getNativePtr();
        }
        if (depthTexture instanceof com.criticalrange.bassalt.texture.BassaltTextureView) {
            depthPtr = ((com.criticalrange.bassalt.texture.BassaltTextureView) depthTexture).getNativePtr();
        }

        int clear = clearColor.orElse(0xFF000000); // Opaque black default
        float depthVal = (float) clearDepth.orElse(1.0);

        // TODO: get proper width/height from textures
        currentRenderPass = device.beginRenderPass(
            device.getNativePtr(),
            colorPtr,
            depthPtr,
            clear,
            depthVal,
            0,
            1920,
            1080
        );

        isActive = true;
        return new BassaltRenderPass(device, currentRenderPass);
    }

    @Override
    public void writeToTexture(GpuTexture destination, NativeImage source) {
        writeToTexture(destination, source, 0, 0, 0, 0,
            source.getWidth(), source.getHeight(), 0, 0);
    }

    @Override
    public void writeToTexture(
        GpuTexture destination,
        NativeImage source,
        int mipLevel,
        int depthOrLayer,
        int destX,
        int destY,
        int width,
        int height,
        int sourceX,
        int sourceY
    ) {
        // Extract pixel data from NativeImage
        byte[] pixels = new byte[width * height * 4]; // Assume RGBA8 for now
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                int srcX = sourceX + x;
                int srcY = sourceY + y;
                int abgr = source.getPixel(srcX, srcY);

                // Convert ABGR to RGBA
                int offset = (y * width + x) * 4;
                pixels[offset] = (byte) ((abgr >> 0) & 0xFF);  // R
                pixels[offset + 1] = (byte) ((abgr >> 8) & 0xFF);  // G
                pixels[offset + 2] = (byte) ((abgr >> 16) & 0xFF); // B
                pixels[offset + 3] = (byte) ((abgr >> 24) & 0xFF); // A
            }
        }

        long texturePtr = ((BassaltTexture) destination).getNativePtr();
        writeToTexture0(device.getNativePtr(), texturePtr, pixels,
            mipLevel, depthOrLayer, destX, destY, width, height,
            destination.getFormat().ordinal());
    }

    @Override
    public void writeToTexture(
        GpuTexture destination,
        ByteBuffer source,
        NativeImage.Format format,
        int mipLevel,
        int depthOrLayer,
        int destX,
        int destY,
        int width,
        int height
    ) {
        byte[] data = new byte[source.remaining()];
        source.get(data);

        long texturePtr = ((BassaltTexture) destination).getNativePtr();
        writeToTexture0(device.getNativePtr(), texturePtr, data,
            mipLevel, depthOrLayer, destX, destY, width, height,
            format.ordinal());
    }

    @Override
    public void writeToBuffer(GpuBufferSlice destination, ByteBuffer data) {
        byte[] arr = new byte[data.remaining()];
        data.get(arr);
        long bufferPtr = ((com.criticalrange.bassalt.buffer.BassaltBuffer) destination.buffer()).getNativePtr();
        BassaltDevice.writeBuffer(device.getNativePtr(), bufferPtr, arr, destination.offset());
    }

    @Override
    public GpuBuffer.@Nullable MappedView mapBuffer(GpuBuffer buffer, boolean read, boolean write) {
        // TODO: implement proper buffer mapping using wgpu's map_buffer API
        throw new UnsupportedOperationException("Buffer mapping not yet implemented");
    }

    @Override
    public GpuBuffer.@Nullable MappedView mapBuffer(GpuBufferSlice buffer, boolean read, boolean write) {
        // TODO: implement proper buffer mapping using wgpu's map_buffer API
        throw new UnsupportedOperationException("Buffer mapping not yet implemented");
    }

    @Override
    public void clearColorTexture(GpuTexture texture, int clearColor) {
        // TODO: implement color texture clear using wgpu's clear_color_texture
    }

    @Override
    public void clearDepthTexture(GpuTexture texture, double depth) {
        // TODO: implement depth texture clear using wgpu's clear_depth_texture
    }

    @Override
    public void clearColorAndDepthTextures(GpuTexture colorTexture, int clearColor, GpuTexture depthTexture,
                                           double clearDepth) {
        // TODO: implement combined color/depth clear (no region)
    }

    @Override
    public void clearColorAndDepthTextures(GpuTexture colorTexture, int clearColor, GpuTexture depthTexture,
                                           double clearDepth, int x, int y, int width, int height) {
        // TODO: implement combined color/depth clear (with region)
    }

    @Override
    public void copyToBuffer(GpuBufferSlice source, GpuBufferSlice target) {
        long srcPtr = ((com.criticalrange.bassalt.buffer.BassaltBuffer) source.buffer()).getNativePtr();
        long dstPtr = ((com.criticalrange.bassalt.buffer.BassaltBuffer) target.buffer()).getNativePtr();
        long size = Math.min(source.length(), target.length());

        copyToBuffer0(device.getNativePtr(), srcPtr, dstPtr,
            source.offset(), target.offset(), size);
    }

    @Override
    public void copyTextureToBuffer(
        GpuTexture source,
        GpuBuffer destination,
        long offset,
        Runnable callback,
        int mipLevel
    ) {
        long texturePtr = ((BassaltTexture) source).getNativePtr();
        long bufferPtr = ((com.criticalrange.bassalt.buffer.BassaltBuffer) destination).getNativePtr();

        // Submit async copy
        copyTextureToBuffer0(device.getNativePtr(), texturePtr, bufferPtr,
            offset, mipLevel, source.getWidth(mipLevel), source.getHeight(mipLevel));

        // Run callback (in a real implementation, this should be called when the copy completes)
        if (callback != null) {
            callback.run();
        }
    }

    @Override
    public void copyTextureToBuffer(
        GpuTexture source,
        GpuBuffer destination,
        long offset,
        Runnable callback,
        int mipLevel,
        int x,
        int y,
        int width,
        int height
    ) {
        long texturePtr = ((BassaltTexture) source).getNativePtr();
        long bufferPtr = ((com.criticalrange.bassalt.buffer.BassaltBuffer) destination).getNativePtr();

        // Submit async copy with region
        copyTextureToBuffer0(device.getNativePtr(), texturePtr, bufferPtr,
            offset, mipLevel, width, height);

        // Run callback (in a real implementation, this should be called when the copy completes)
        if (callback != null) {
            callback.run();
        }
    }

    @Override
    public void presentTexture(GpuTextureView textureView) {
        // Texture presentation is handled via swapchain, not individual textures
        device.presentFrame();
    }

    @Override
    public GpuFence createFence() {
        // TODO: implement proper fence support using wgpu's fence/signaling API
        return new GpuFence() {
            private volatile boolean signaled = false;

            @Override
            public void close() {
                signaled = true;
            }

            @Override
            public boolean awaitCompletion(long timeoutMs) {
                try {
                    Thread.sleep(Math.min(timeoutMs, 100));
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
                signaled = true;
                return true;
            }
        };
    }

    @Override
    public GpuQuery timerQueryBegin() {
        // TODO: implement timer queries using wgpu's timestamp queries
        return new GpuQuery() {
            private final long startTime = System.nanoTime();

            @Override
            public void close() {}

            @Override
            public OptionalLong getValue() {
                return OptionalLong.of(startTime);
            }
        };
    }

    public void timerQueryEnd(GpuQuery query) {
        // TODO: end timer query
    }

    @Override
    public void copyTextureToTexture(
        GpuTexture source,
        GpuTexture destination,
        int mipLevel,
        int destX,
        int destY,
        int sourceX,
        int sourceY,
        int width,
        int height
    ) {
        // TODO: implement texture-to-texture copy using wgpu's copy_texture_to_texture
    }

    public void finish() {
        if (currentRenderPass != 0) {
            device.endRenderPass(device.getNativePtr(), currentRenderPass);
            currentRenderPass = 0;
        }
        isActive = false;
        finished = true;
    }

    public BassaltDevice getDevice() {
        return device;
    }

    public long getCurrentRenderPass() {
        return currentRenderPass;
    }

    public void setCurrentRenderPass(long renderPass) {
        this.currentRenderPass = renderPass;
    }
}
