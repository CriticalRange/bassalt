package com.criticalrange.bassalt.texture;

import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.textures.TextureFormat;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.jspecify.annotations.Nullable;

/**
 * Bassalt Texture View - Wraps a native WebGPU texture view
 */
@Environment(EnvType.CLIENT)
public class BassaltTextureView extends GpuTextureView {

    private final long nativePtr;
    private boolean closed = false;

    public BassaltTextureView(GpuTexture texture, long nativePtr) {
        super(texture, 0, 1);
        this.nativePtr = nativePtr;
    }

    /**
     * Create a texture view with specific mip level settings
     */
    public BassaltTextureView(GpuTexture texture, long nativePtr, int baseMipLevel, int mipLevels) {
        super(texture, baseMipLevel, mipLevels);
        this.nativePtr = nativePtr;
    }

    @Override
    public void close() {
        if (!closed) {
            // Texture views are typically managed by their parent texture
            // So we don't explicitly destroy them here
            closed = true;
        }
    }

    public long getNativePtr() {
        return nativePtr;
    }

    public boolean isClosed() {
        return closed;
    }
}
