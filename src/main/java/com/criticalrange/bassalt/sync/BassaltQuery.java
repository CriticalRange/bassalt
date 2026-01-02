package com.criticalrange.bassalt.sync;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.mojang.blaze3d.systems.GpuQuery;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

import java.util.OptionalLong;

/**
 * Bassalt Query - Implements GPU timer queries using wgpu timestamp queries.
 *
 * WebGPU supports timestamp queries for measuring GPU execution time.
 * This requires the "timestamp-query" feature to be enabled on the device.
 */
@Environment(EnvType.CLIENT)
public class BassaltQuery implements GpuQuery {

    private final BassaltDevice device;
    private final long nativePtr;
    private final long startTimestamp;
    private long endTimestamp;
    private boolean closed = false;
    private boolean ended = false;

    // Native methods
    private static native long createTimestampQuery(long devicePtr);
    private static native void destroyTimestampQuery(long devicePtr, long queryPtr);
    private static native long getTimestampValue(long devicePtr, long queryPtr);
    private static native boolean isTimestampQuerySupported(long devicePtr);

    public BassaltQuery(BassaltDevice device) {
        this.device = device;

        // Check if timestamp queries are supported
        if (isTimestampQuerySupported(device.getNativePtr())) {
            this.nativePtr = createTimestampQuery(device.getNativePtr());
            this.startTimestamp = 0; // Will be set by GPU
        } else {
            // Fallback to CPU timing if not supported
            this.nativePtr = 0;
            this.startTimestamp = System.nanoTime();
        }
        this.endTimestamp = 0;
    }

    /**
     * Mark the query as ended. This records the end timestamp.
     * For GPU queries, the actual timestamp will be resolved when getValue() is called.
     * For CPU fallback, this records the current system time.
     */
    public void end() {
        if (!ended) {
            if (nativePtr == 0) {
                // CPU fallback: record end time
                endTimestamp = System.nanoTime();
            }
            // For GPU queries, the timestamp is written by the GPU
            ended = true;
        }
    }

    @Override
    public OptionalLong getValue() {
        if (closed || !ended) {
            return OptionalLong.empty();
        }

        if (nativePtr != 0) {
            // Get GPU timestamp
            long value = getTimestampValue(device.getNativePtr(), nativePtr);
            if (value >= 0) {
                return OptionalLong.of(value);
            }
            return OptionalLong.empty();
        } else {
            // Return CPU elapsed time as fallback (in nanoseconds)
            return OptionalLong.of(endTimestamp - startTimestamp);
        }
    }

    @Override
    public void close() {
        if (!closed && nativePtr != 0) {
            destroyTimestampQuery(device.getNativePtr(), nativePtr);
        }
        closed = true;
    }
}
