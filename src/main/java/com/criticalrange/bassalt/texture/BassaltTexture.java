package com.criticalrange.bassalt.texture;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.TextureFormat;

/**
 * Bassalt Texture - Implements Minecraft's GpuTexture interface
 */
public class BassaltTexture extends GpuTexture {

    private final BassaltDevice device;
    private final long nativePtr;
    private boolean closed = false;

    public BassaltTexture(BassaltDevice device, long nativePtr, TextureFormat format, int width, int height) {
        super(0, "BassaltTexture", format, width, height, 1, 1);
        this.device = device;
        this.nativePtr = nativePtr;
    }

    @Override
    public boolean isClosed() {
        return closed;
    }

    @Override
    public void close() {
        if (!closed) {
            device.destroyNativeTexture(nativePtr);
            closed = true;
        }
    }

    public long getNativePtr() {
        return nativePtr;
    }
}
