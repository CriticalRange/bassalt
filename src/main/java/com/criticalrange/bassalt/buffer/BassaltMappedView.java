package com.criticalrange.bassalt.buffer;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.criticalrange.bassalt.backend.BassaltDevice;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

import java.nio.ByteBuffer;

/**
 * Bassalt Mapped Buffer View - Provides a CPU-accessible shadow buffer for mapped GPU buffers
 *
 * WebGPU doesn't support synchronous buffer mapping like OpenGL. Instead, we use a "shadow buffer"
 * approach: allocate a ByteBuffer in system memory, return that to the caller, and when the view
 * is closed, copy the shadow buffer data to the GPU.
 */
@Environment(EnvType.CLIENT)
public class BassaltMappedView implements GpuBuffer.MappedView {

    private final BassaltDevice device;
    private final BassaltBuffer buffer;
    private final ByteBuffer shadowBuffer;
    private final long offset;
    private final boolean write;
    private boolean closed = false;

    /**
     * Create a mapped view for a buffer
     *
     * @param device The Bassalt device
     * @param buffer The buffer being mapped
     * @param offset Offset into the buffer
     * @param size Size of the mapped region
     * @param write Whether the buffer is mapped for writing (needs flush on close)
     */
    public BassaltMappedView(BassaltDevice device, BassaltBuffer buffer, long offset, long size, boolean write) {
        this.device = device;
        this.buffer = buffer;
        this.offset = offset;
        this.write = write;

        // Allocate a shadow buffer in system memory
        // In a production implementation, we might want to cache this shadow buffer
        // to avoid repeated allocations, but for now this is sufficient
        this.shadowBuffer = ByteBuffer.allocateDirect((int) size);

        // DEBUG: Log shadow buffer allocation and check initial contents
        System.out.println("[Bassalt DEBUG] MappedView allocated: bufferPtr=" + buffer.getNativePtr() +
                         ", offset=" + offset + ", size=" + size + ", write=" + write);

        // Check if buffer is actually zero-initialized
        if (size >= 80) {
            java.nio.ByteBuffer bb = shadowBuffer.order(java.nio.ByteOrder.LITTLE_ENDIAN);
            float initialR = bb.getFloat(64);
            float initialG = bb.getFloat(68);
            float initialB = bb.getFloat(72);
            float initialA = bb.getFloat(76);
            System.out.println("[Bassalt DEBUG]   Initial ColorModulator at offset 64: [" +
                             initialR + ", " + initialG + ", " + initialB + ", " + initialA + "]");
        }
    }

    @Override
    public ByteBuffer data() {
        if (closed) {
            throw new IllegalStateException("Mapped view has been closed");
        }

        // DEBUG: Log every time data() is called
        System.out.println("[Bassalt DEBUG] MappedView.data() called: bufferPtr=" + buffer.getNativePtr() +
                         ", remaining=" + shadowBuffer.remaining() + ", position=" + shadowBuffer.position());

        return shadowBuffer;
    }

    @Override
    public void close() {
        if (closed) {
            return;
        }
        closed = true;

        // If mapped for writing, copy the shadow buffer data to the GPU
        if (write && shadowBuffer != null) {
            // DEBUG: Log buffer state before flip
            System.out.println("[Bassalt DEBUG] MappedView.close(): before flip - position=" +
                             shadowBuffer.position() + ", limit=" + shadowBuffer.limit() +
                             ", capacity=" + shadowBuffer.capacity());

            shadowBuffer.flip();

            byte[] data = new byte[shadowBuffer.remaining()];
            shadowBuffer.get(data);

            // DEBUG: Log what we're about to write
            if (data.length >= 80) {
                java.nio.ByteBuffer bb = java.nio.ByteBuffer.wrap(data).order(java.nio.ByteOrder.LITTLE_ENDIAN);
                float colorModR = bb.getFloat(64);
                float colorModG = bb.getFloat(68);
                float colorModB = bb.getFloat(72);
                float colorModA = bb.getFloat(76);
                System.out.println("[Bassalt DEBUG] MappedView.close(): bufferPtr=" + buffer.getNativePtr() +
                                 ", data.length=" + data.length +
                                 ", writing ColorModulator=[" + colorModR + ", " + colorModG + ", " + colorModB + ", " + colorModA + "]");

                // Also log first few floats to see pattern
                System.out.println("[Bassalt DEBUG]   First 4 floats (ModelViewMat): [" +
                                 bb.getFloat(0) + ", " + bb.getFloat(4) + ", " + bb.getFloat(8) + ", " + bb.getFloat(12) + "]");
            }

            BassaltDevice.writeBuffer(
                device.getNativePtr(),
                buffer.getNativePtr(),
                data,
                offset
            );
        }

        // Note: we don't close the buffer itself, just the mapped view
    }

    @Override
    protected void finalize() throws Throwable {
        try {
            if (!closed) {
                close();
            }
        } finally {
            super.finalize();
        }
    }
}
