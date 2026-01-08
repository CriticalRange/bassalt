package com.criticalrange.bassalt.buffer;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.criticalrange.bassalt.backend.BassaltDevice;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

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

    private static final Logger LOGGER = LogManager.getLogger("Bassalt");

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
     * @param shadowBuffer The cached shadow buffer from BassaltBuffer
     * @param offset Offset into the buffer
     * @param size Size of the mapped region
     * @param write Whether the buffer is mapped for writing (needs flush on close)
     */
    public BassaltMappedView(BassaltDevice device, BassaltBuffer buffer, ByteBuffer shadowBuffer, long offset, long size, boolean write) {
        this.device = device;
        this.buffer = buffer;
        this.offset = offset;
        this.write = write;

        // Use the cached shadow buffer from BassaltBuffer
        // This preserves data across map()/unmap() calls, just like OpenGL's persistent buffers
        this.shadowBuffer = shadowBuffer;

        // CRITICAL: Ensure LITTLE_ENDIAN byte order for correct float/int data interpretation
        // This was previously set as a side effect of debug code, but is essential for correctness
        shadowBuffer.order(java.nio.ByteOrder.LITTLE_ENDIAN);
    }

    @Override
    public ByteBuffer data() {
        if (closed) {
            throw new IllegalStateException("Mapped view has been closed");
        }
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
            // CRITICAL FIX: Only upload what MC actually wrote!
            // Find the last non-zero byte to determine the actual data size
            int oldPosition = shadowBuffer.position();
            int oldLimit = shadowBuffer.limit();

            // Create a copy to scan for trailing zeros without modifying position
            byte[] tempScan = new byte[Math.min(shadowBuffer.capacity(), 4096)]; // Scan first 4KB
            shadowBuffer.position(0);
            shadowBuffer.limit(tempScan.length);
            shadowBuffer.get(tempScan);
            shadowBuffer.position(oldPosition);
            shadowBuffer.limit(oldLimit);

            // Find the last non-zero byte in the scanned portion
            int actualDataSize = tempScan.length;
            while (actualDataSize > 0 && tempScan[actualDataSize - 1] == 0) {
                actualDataSize--;
            }

            // If the entire scanned portion is zeros, check if there's more data beyond
            if (actualDataSize == 0 && shadowBuffer.capacity() > tempScan.length) {
                actualDataSize = shadowBuffer.capacity(); // Upload everything (shouldn't happen)
            }

            // CRITICAL: Round up to 4-byte alignment (WebGPU requirement)
            // Vertex buffers also benefit from 16-byte alignment (vertex size), but 4 is minimum
            int alignedSize = (actualDataSize + 3) & ~3;
            if (alignedSize > shadowBuffer.capacity()) {
                alignedSize = shadowBuffer.capacity();
            }

            // Read only the actual data (with 4-byte alignment padding)
            shadowBuffer.position(0);
            shadowBuffer.limit(alignedSize);

            byte[] data = new byte[alignedSize];
            shadowBuffer.get(data);

            // Restore old position/limit (though we're closing anyway)
            shadowBuffer.position(oldPosition);
            shadowBuffer.limit(oldLimit);

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
