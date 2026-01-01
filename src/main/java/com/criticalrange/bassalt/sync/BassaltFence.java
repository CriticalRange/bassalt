package com.criticalrange.bassalt.sync;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.mojang.blaze3d.buffers.GpuFence;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;

/**
 * Bassalt Fence - Implements GPU synchronization using wgpu queue submission tracking.
 * 
 * WebGPU doesn't have explicit fences like Vulkan, but we can use queue.onSubmittedWorkDone()
 * or poll the device to check for completion. For simplicity, we track submission index
 * and poll the device.
 */
@Environment(EnvType.CLIENT)
public class BassaltFence implements GpuFence {
    
    private final BassaltDevice device;
    private final long submissionIndex;
    private volatile boolean completed = false;
    private boolean closed = false;
    
    // Native methods
    private static native long getSubmissionIndex(long devicePtr);
    private static native boolean pollDevice(long devicePtr, boolean wait);
    private static native boolean isWorkComplete(long devicePtr, long submissionIndex);
    
    public BassaltFence(BassaltDevice device) {
        this.device = device;
        // Get current submission index when fence is created
        this.submissionIndex = getSubmissionIndex(device.getNativePtr());
    }
    
    @Override
    public boolean awaitCompletion(long timeoutMs) {
        if (completed || closed) {
            return true;
        }
        
        long startTime = System.currentTimeMillis();
        long deadline = startTime + timeoutMs;
        
        // Poll until complete or timeout
        while (System.currentTimeMillis() < deadline) {
            // Poll device to process completed work
            pollDevice(device.getNativePtr(), false);
            
            // Check if our submission is complete
            if (isWorkComplete(device.getNativePtr(), submissionIndex)) {
                completed = true;
                return true;
            }
            
            // Small sleep to avoid busy-waiting
            try {
                Thread.sleep(1);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
        }
        
        // Final check
        completed = isWorkComplete(device.getNativePtr(), submissionIndex);
        return completed;
    }
    
    @Override
    public void close() {
        closed = true;
        completed = true;
    }
    
    public boolean isCompleted() {
        return completed;
    }
}
