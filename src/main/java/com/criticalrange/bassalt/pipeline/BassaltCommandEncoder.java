package com.criticalrange.bassalt.pipeline;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.criticalrange.bassalt.buffer.BassaltBuffer;
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

import com.criticalrange.bassalt.sync.BassaltFence;
import com.criticalrange.bassalt.sync.BassaltQuery;

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
    private static native void clearColorTexture0(long devicePtr, long texturePtr, int clearColor);
    private static native void clearDepthTexture0(long devicePtr, long texturePtr, float clearDepth);
    private static native void clearColorAndDepthTextures0(long devicePtr, long colorTexturePtr, int clearColor,
                                                           long depthTexturePtr, float clearDepth,
                                                           int x, int y, int width, int height);
    private static native void copyTextureToTexture0(long devicePtr, long srcTexturePtr, long dstTexturePtr,
                                                      int mipLevel, int destX, int destY, int sourceX, int sourceY,
                                                      int width, int height);

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
        int width = 854;  // Default fallback
        int height = 480; // Default fallback

        if (colorTexture instanceof com.criticalrange.bassalt.texture.BassaltTextureView) {
            colorPtr = ((com.criticalrange.bassalt.texture.BassaltTextureView) colorTexture).getNativePtr();
            // Get actual dimensions from the texture
            width = colorTexture.texture().getWidth(0);
            height = colorTexture.texture().getHeight(0);
        }
        if (depthTexture instanceof com.criticalrange.bassalt.texture.BassaltTextureView) {
            depthPtr = ((com.criticalrange.bassalt.texture.BassaltTextureView) depthTexture).getNativePtr();
        }

        boolean shouldClearColor = clearColor.isPresent();
        boolean shouldClearDepth = clearDepth.isPresent();
        int clear = clearColor.orElse(0xFF000000); // Opaque black default
        float depthVal = (float) clearDepth.orElse(1.0);

        currentRenderPass = device.beginRenderPass(
            device.getNativePtr(),
            colorPtr,
            depthPtr,
            shouldClearColor,
            clear,
            shouldClearDepth,
            depthVal,
            0,
            width,
            height
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
        BassaltBuffer bassaltBuffer = (BassaltBuffer) buffer;
        return new com.criticalrange.bassalt.buffer.BassaltMappedView(
            device,
            bassaltBuffer,
            0,  // offset
            bassaltBuffer.size(),
            write
        );
    }

    @Override
    public GpuBuffer.@Nullable MappedView mapBuffer(GpuBufferSlice bufferSlice, boolean read, boolean write) {
        BassaltBuffer bassaltBuffer = (BassaltBuffer) bufferSlice.buffer();
        return new com.criticalrange.bassalt.buffer.BassaltMappedView(
            device,
            bassaltBuffer,
            bufferSlice.offset(),
            bufferSlice.length(),
            write
        );
    }

    @Override
    public void clearColorTexture(GpuTexture texture, int clearColor) {
        long texturePtr = ((BassaltTexture) texture).getNativePtr();
        clearColorTexture0(device.getNativePtr(), texturePtr, clearColor);
    }

    @Override
    public void clearDepthTexture(GpuTexture texture, double depth) {
        long texturePtr = ((BassaltTexture) texture).getNativePtr();
        clearDepthTexture0(device.getNativePtr(), texturePtr, (float) depth);
    }

    @Override
    public void clearColorAndDepthTextures(GpuTexture colorTexture, int clearColor, GpuTexture depthTexture,
                                           double clearDepth) {
        long colorPtr = ((BassaltTexture) colorTexture).getNativePtr();
        long depthPtr = ((BassaltTexture) depthTexture).getNativePtr();
        clearColorAndDepthTextures0(device.getNativePtr(), colorPtr, clearColor, depthPtr, (float) clearDepth,
                                     0, 0, colorTexture.getWidth(0), colorTexture.getHeight(0));
    }

    @Override
    public void clearColorAndDepthTextures(GpuTexture colorTexture, int clearColor, GpuTexture depthTexture,
                                           double clearDepth, int x, int y, int width, int height) {
        long colorPtr = ((BassaltTexture) colorTexture).getNativePtr();
        long depthPtr = ((BassaltTexture) depthTexture).getNativePtr();
        clearColorAndDepthTextures0(device.getNativePtr(), colorPtr, clearColor, depthPtr, (float) clearDepth,
                                     x, y, width, height);
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
        return new BassaltFence(device);
    }

    @Override
    public GpuQuery timerQueryBegin() {
        return new BassaltQuery(device);
    }

    public void timerQueryEnd(GpuQuery query) {
        if (query instanceof BassaltQuery bassaltQuery) {
            // Finalize the timestamp query
            // The query will be resolved when getValue() is called
            bassaltQuery.end();
        }
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
        long srcPtr = ((BassaltTexture) source).getNativePtr();
        long dstPtr = ((BassaltTexture) destination).getNativePtr();
        copyTextureToTexture0(device.getNativePtr(), srcPtr, dstPtr,
                               mipLevel, destX, destY, sourceX, sourceY, width, height);
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
