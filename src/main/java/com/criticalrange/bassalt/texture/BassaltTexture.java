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

    public BassaltTexture(BassaltDevice device, long nativePtr, int usage, String label, 
                          TextureFormat format, int width, int height, int depthOrLayers, int mipLevels) {
        super(usage, label, format, width, height, depthOrLayers, mipLevels);
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
