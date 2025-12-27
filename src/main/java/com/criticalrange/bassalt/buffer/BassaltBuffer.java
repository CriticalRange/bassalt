package com.criticalrange.bassalt.buffer;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.mojang.blaze3d.buffers.GpuBuffer;

/**
 * Bassalt Buffer - Implements Minecraft's GpuBuffer interface
 */
public class BassaltBuffer extends GpuBuffer {

    private final BassaltDevice device;
    private final long nativePtr;
    private boolean closed = false;

    public BassaltBuffer(BassaltDevice device, long nativePtr, int usage, long size) {
        super(usage, size);
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
            device.destroyNativeBuffer(nativePtr);
            closed = true;
        }
    }

    public long getNativePtr() {
        return nativePtr;
    }
}
