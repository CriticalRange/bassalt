package com.criticalrange.bassalt.buffer;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.mojang.blaze3d.buffers.GpuBuffer;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

/**
 * Bassalt Buffer - Implements Minecraft's GpuBuffer interface
 *
 * Uses shadow buffer caching similar to OpenGL's persistent buffers:
 * - One shadow buffer per GPU buffer that persists across map()/unmap() calls
 * - Data accumulates across multiple writes per frame
 * - Preserved across frames just like OpenGL's persistent buffers
 * - Only discarded when the GPU buffer itself is closed
 */
public class BassaltBuffer extends GpuBuffer {

    private static final Logger LOGGER = LogManager.getLogger("Bassalt");

    private final BassaltDevice device;
    private final long nativePtr;
    private boolean closed = false;

    // Shadow buffer cache - persists across map()/unmap() calls
    // Key = offset, Value = shadow buffer for that slice
    // NOTE: Each BassaltBuffer has its own cache, so shadow buffers are NOT
    // shared across different GPU buffers (even if they have the same offset)
    private java.util.HashMap<Long, ByteBuffer> shadowBuffers = new java.util.HashMap<>();
    // Track which shadow buffers have been initialized (zeroed)
    private java.util.HashSet<Long> shadowBuffersInitialized = new java.util.HashSet<>();

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

            // Clean up all shadow buffers
            shadowBuffers.clear();
            shadowBuffers = null;
            shadowBuffersInitialized.clear();
            shadowBuffersInitialized = null;
        }
    }

    public long getNativePtr() {
        return nativePtr;
    }

    /**
     * Get or create the shadow buffer for a specific slice of this GPU buffer.
     * Each slice (at a specific offset) gets its own shadow buffer that persists
     * across map()/unmap() calls, similar to OpenGL's persistent buffers.
     *
     * Based on wgpu-mc implementation: shadow buffers are zero-initialized
     * when first created to prevent stale data from ring buffer rotation.
     *
     * @param offset Offset into the GPU buffer
     * @param size Size of the mapped slice
     * @return A ByteBuffer slice that can be used for CPU-side data access
     */
    public ByteBuffer getOrCreateShadowBuffer(long offset, long size) {
        // Use offset as the key to cache shadow buffers per slice
        Long key = Long.valueOf(offset);

        ByteBuffer shadowBuffer = shadowBuffers.get(key);
        boolean needsInit = false;

        if (shadowBuffer == null || shadowBuffer.capacity() < (int) size) {
            // Allocate shadow buffer for this slice
            // IMPORTANT: Zero-initialize like wgpu-mc does with memCalloc
            // This prevents stale data from previous ring buffer rotations
            shadowBuffer = ByteBuffer.allocateDirect((int) size);
            shadowBuffers.put(key, shadowBuffer);
            needsInit = true;
        }

        // CRITICAL FIX: Always zero the shadow buffer on EVERY map() call!
        // This prevents stale data from previous frames from being uploaded.
        // When ring buffers rotate after 3 frames, unwritten portions would have
        // old data, causing flashing triangles.
        for (int i = 0; i < shadowBuffer.capacity(); i++) {
            shadowBuffer.put(i, (byte) 0);
        }

        // Return a slice view of the shadow buffer (like wgpu-mc's buf.slice())
        ByteBuffer view = shadowBuffer.slice();
        return view;
    }
}
