package com.criticalrange.bassalt.pipeline;

import com.criticalrange.bassalt.backend.BassaltDevice;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

/**
 * Bassalt Compiled Render Pipeline - Represents a precompiled WebGPU render pipeline
 *
 * This class wraps a native WebGPU render pipeline ID and provides lifecycle management.
 * Compiled pipelines can be reused across multiple render passes for better performance.
 */
@Environment(EnvType.CLIENT)
public class BassaltCompiledRenderPipeline implements com.mojang.blaze3d.pipeline.CompiledRenderPipeline {

    private final long nativePipelinePtr;
    private final BassaltDevice device;
    private boolean closed = false;

    /**
     * Create a compiled pipeline from a native pointer
     *
     * @param device The device that created this pipeline
     * @param nativePipelinePtr The native pipeline ID
     */
    public BassaltCompiledRenderPipeline(BassaltDevice device, long nativePipelinePtr) {
        this.device = device;
        this.nativePipelinePtr = nativePipelinePtr;
    }

    /**
     * Get the native pipeline pointer
     *
     * @return The native WebGPU render pipeline ID
     */
    public long getNativePtr() {
        return nativePipelinePtr;
    }

    /**
     * Check if this pipeline is still valid (not closed)
     *
     * @return true if the pipeline is valid
     */
    public boolean isValid() {
        return !closed && nativePipelinePtr != 0;
    }

    /**
     * Check if this pipeline is closed
     *
     * @return true if the pipeline is closed
     */
    public boolean isClosed() {
        return closed;
    }

    public void close() {
        if (!closed) {
            // Native pipeline cleanup happens when the device is closed
            // WebGPU pipelines are reference-counted internally
            closed = true;
        }
    }

    public String toString() {
        return "BassaltCompiledRenderPipeline{" +
            "ptr=0x" + Long.toHexString(nativePipelinePtr) +
            ", valid=" + isValid() +
            '}';
    }
}
