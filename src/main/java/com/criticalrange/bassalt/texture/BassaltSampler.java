package com.criticalrange.bassalt.texture;

import com.mojang.blaze3d.textures.AddressMode;
import com.mojang.blaze3d.textures.FilterMode;
import com.mojang.blaze3d.textures.GpuSampler;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

import java.util.OptionalDouble;

/**
 * Bassalt Sampler - Wraps a native WebGPU sampler
 */
@Environment(EnvType.CLIENT)
public class BassaltSampler extends GpuSampler {

    private final long nativePtr;
    private final AddressMode addressModeU;
    private final AddressMode addressModeV;
    private final FilterMode minFilter;
    private final FilterMode magFilter;
    private final int maxAnisotropy;
    private final double maxLod;
    private boolean closed = false;

    public BassaltSampler(long nativePtr, AddressMode addressModeU, AddressMode addressModeV,
                         FilterMode minFilter, FilterMode magFilter,
                         int maxAnisotropy, double maxLod) {
        this.nativePtr = nativePtr;
        this.addressModeU = addressModeU;
        this.addressModeV = addressModeV;
        this.minFilter = minFilter;
        this.magFilter = magFilter;
        this.maxAnisotropy = maxAnisotropy;
        this.maxLod = maxLod;
    }

    public long getNativePtr() {
        return nativePtr;
    }

    @Override
    public AddressMode getAddressModeU() {
        return addressModeU;
    }

    @Override
    public AddressMode getAddressModeV() {
        return addressModeV;
    }

    @Override
    public FilterMode getMinFilter() {
        return minFilter;
    }

    @Override
    public FilterMode getMagFilter() {
        return magFilter;
    }

    @Override
    public int getMaxAnisotropy() {
        return maxAnisotropy;
    }

    @Override
    public OptionalDouble getMaxLod() {
        return OptionalDouble.of(maxLod);
    }

    @Override
    public void close() {
        closed = true;
    }

    public boolean isClosed() {
        return closed;
    }
}
