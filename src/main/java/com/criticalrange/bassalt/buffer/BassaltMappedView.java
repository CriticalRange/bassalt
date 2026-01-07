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

        // DEBUG: Log shadow buffer usage and check initial contents
        System.out.println("[Bassalt DEBUG] MappedView using cached shadow buffer: bufferPtr=" + buffer.getNativePtr() +
                         ", offset=" + offset + ", size=" + size + ", write=" + write +
                         ", shadowBuffer.capacity=" + shadowBuffer.capacity());

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
        System.out.println("[Bassalt DEBUG] MappedView.close() ENTRY: bufferPtr=" + buffer.getNativePtr() +
                         ", closed=" + closed + ", write=" + write + ", shadowBuffer=" + (shadowBuffer != null));

        if (closed) {
            System.out.println("[Bassalt DEBUG] MappedView.close() EARLY RETURN (already closed): bufferPtr=" + buffer.getNativePtr());
            return;
        }
        closed = true;

        // If mapped for writing, copy the shadow buffer data to the GPU
        if (write && shadowBuffer != null) {
            // DEBUG: Log buffer state before copy
            System.out.println("[Bassalt DEBUG] MappedView.close(): position=" +
                             shadowBuffer.position() + ", limit=" + shadowBuffer.limit() +
                             ", capacity=" + shadowBuffer.capacity());

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

            System.out.println("[Bassalt DEBUG] MappedView.close(): capacity=" + shadowBuffer.capacity() + ", actualDataSize=" + actualDataSize + ", alignedSize=" + alignedSize + " (4-byte aligned)");

            // Read only the actual data (with 4-byte alignment padding)
            shadowBuffer.position(0);
            shadowBuffer.limit(alignedSize);

            byte[] data = new byte[alignedSize];
            shadowBuffer.get(data);

            // Restore old position/limit (though we're closing anyway)
            shadowBuffer.position(oldPosition);
            shadowBuffer.limit(oldLimit);

            // DEBUG: Log vertex data being uploaded
            System.out.println("[Bassalt DEBUG] MappedView.close(): bufferPtr=" + buffer.getNativePtr() +
                             ", GPU offset=" + offset + ", uploading " + data.length + " bytes");

            // For POSITION_COLOR format (16 bytes/vertex), log vertex data
            int vertexSize = 16; // POSITION_COLOR: 12 bytes position + 4 bytes color
            int vertexCount = data.length / vertexSize;
            System.out.println("[Bassalt DEBUG]   Vertices to upload: " + vertexCount + " (bytes=" + data.length + ", vertexSize=" + vertexSize + ")");

            if (vertexCount > 0 && data.length >= vertexSize) {
                java.nio.ByteBuffer bb = java.nio.ByteBuffer.wrap(data).order(java.nio.ByteOrder.nativeOrder());
                System.out.println("[Bassalt DEBUG]   First vertex: pos=(" + bb.getFloat(0) + ", " + bb.getFloat(4) + ", " + bb.getFloat(8) + "), color=" + bb.getInt(12));

                // Log last vertex if we have more than one
                if (vertexCount > 1) {
                    int lastOffset = (vertexCount - 1) * vertexSize;
                    System.out.println("[Bassalt DEBUG]   Last vertex #" + (vertexCount-1) + ": pos=(" +
                        bb.getFloat(lastOffset) + ", " + bb.getFloat(lastOffset + 4) + ", " + bb.getFloat(lastOffset + 8) +
                        "), color=" + bb.getInt(lastOffset + 12));
                }

                // Check if any vertices are all zeros
                int zeroVertices = 0;
                for (int v = 0; v < vertexCount; v++) {
                    int offset = v * vertexSize;
                    float x = bb.getFloat(offset);
                    float y = bb.getFloat(offset + 4);
                    float z = bb.getFloat(offset + 8);
                    int color = bb.getInt(offset + 12);
                    if (x == 0 && y == 0 && z == 0 && color == 0) {
                        zeroVertices++;
                    }
                }
                System.out.println("[Bassalt DEBUG]   Zero vertices: " + zeroVertices + " / " + vertexCount);
            }

            // DEBUG: Log what we're about to write (only if >= 80 bytes to avoid spam)
            if (data.length >= 80) {
                java.nio.ByteBuffer bb = java.nio.ByteBuffer.wrap(data).order(java.nio.ByteOrder.nativeOrder());
                float colorModR = bb.getFloat(64);
                float colorModG = bb.getFloat(68);
                float colorModB = bb.getFloat(72);
                float colorModA = bb.getFloat(76);
                System.out.println("[Bassalt DEBUG] MappedView.close(): bufferPtr=" + buffer.getNativePtr() +
                                 ", GPU offset=" + offset + ", data.length=" + data.length +
                                 ", ColorModulator at shadow[64]=[" + colorModR + ", " + colorModG + ", " + colorModB + ", " + colorModA + "]");

                // Also log what's at the beginning of the buffer to see structure
                System.out.println("[Bassalt DEBUG]   First 4 floats at shadow[0]: [" +
                    bb.getFloat(0) + ", " + bb.getFloat(4) + ", " + bb.getFloat(8) + ", " + bb.getFloat(12) + "]");

                // Log full ModelViewMat (first 16 floats = 4x4 matrix)
                System.out.println("[Bassalt DEBUG]   ModelViewMat (16 floats):");
                for (int row = 0; row < 4; row++) {
                    System.out.println("     Row " + row + ": [" +
                        bb.getFloat(row*16 + 0) + ", " +
                        bb.getFloat(row*16 + 4) + ", " +
                        bb.getFloat(row*16 + 8) + ", " +
                        bb.getFloat(row*16 + 12) + "]");
                }
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
