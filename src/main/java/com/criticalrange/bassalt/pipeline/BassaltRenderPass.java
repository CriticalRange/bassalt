package com.criticalrange.bassalt.pipeline;

import com.criticalrange.bassalt.backend.BassaltDevice;
import com.criticalrange.bassalt.buffer.BassaltBuffer;
import com.criticalrange.bassalt.texture.BassaltSampler;
import com.criticalrange.bassalt.texture.BassaltTextureView;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.textures.GpuSampler;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.VertexFormat;
import net.fabricmc.api.EnvType;
import net.fabricmc.api.Environment;
import org.jspecify.annotations.Nullable;

import java.util.Collection;
import java.util.HashMap;
import java.util.Map;
import java.util.function.BiConsumer;
import java.util.function.Supplier;

/**
 * Bassalt Render Pass - Implements Minecraft's RenderPass interface
 *
 * This class manages the current render pass state including:
 * - Active pipeline
 * - Bound textures and samplers (via bind groups)
 * - Bound vertex and index buffers
 * - Uniform buffers
 */
@Environment(EnvType.CLIENT)
public class BassaltRenderPass implements RenderPass {

    private final BassaltDevice device;
    private final long nativePassPtr;
    private boolean closed = false;

    // Current bind state (for tracking and creating bind groups)
    private final Map<String, TextureBinding> textureBindings = new HashMap<>();
    private final Map<String, UniformBinding> uniformBindings = new HashMap<>();
    private RenderPipeline currentPipeline;
    private long currentPipelineHandle = 0; // Native handle for current pipeline

    // Native methods for bind group management
    // pipeline_handle is required to use the correct bind group layout
    private static native long createBindGroup0(long devicePtr, long renderPassPtr, long pipelineHandle,
            String[] textureNames, long[] textures, long[] samplers,
            String[] uniformNames, long[] uniforms);
    
    // Multi-bind-group support (wgpu-mc style)
    // Returns first non-zero bind group handle as success indicator
    // TODO: Return array of bind group handles when JNI array return is implemented
    private static native long createMultiBindGroups(long devicePtr, long renderPassPtr, long pipelineHandle,
            String[] textureNames, long[] textures, long[] samplers,
            String[] uniformNames, long[] uniforms, long[] uniformOffsets, long[] uniformSizes);

    private static native void setBindGroup0(long devicePtr, long renderPassPtr,
            int index, long bindGroupPtr);

    // Native methods for debug groups and markers
    private static native void pushDebugGroup(long renderPassPtr, String label);
    private static native void popDebugGroup(long renderPassPtr);
    private static native void insertDebugMarker(long renderPassPtr, String label);

    BassaltRenderPass(BassaltDevice device, long nativePassPtr) {
        this.device = device;
        this.nativePassPtr = nativePassPtr;
    }

    @Override
    public void pushDebugGroup(Supplier<String> label) {
        checkClosed();
        pushDebugGroup(nativePassPtr, label.get());
    }

    @Override
    public void popDebugGroup() {
        checkClosed();
        popDebugGroup(nativePassPtr);
    }

    @Override
    public void setPipeline(RenderPipeline pipeline) {
        checkClosed();
        this.currentPipeline = pipeline;

        // Get the compiled pipeline from the device and call native setPipeline
        BassaltCompiledRenderPipeline compiled = (BassaltCompiledRenderPipeline) device.precompilePipeline(pipeline,
                null);
        if (compiled != null && compiled.isValid()) {
            this.currentPipelineHandle = compiled.getNativePtr();
            BassaltDevice.setPipeline(device.getNativePtr(), nativePassPtr, compiled.getNativePtr());
        } else {
            this.currentPipelineHandle = 0;
            // Log warning if pipeline compilation failed
            System.err.println("[Bassalt] Warning: Pipeline compilation failed for " + pipeline.getLocation());
        }
    }

    @Override
    public void bindTexture(@Nullable String name, @Nullable GpuTextureView textureView, @Nullable GpuSampler sampler) {
        checkClosed();
        if (name == null)
            return;

        // Debug: log what we're receiving
        System.out.println("[Bassalt] bindTexture: name=" + name + 
            ", textureView=" + (textureView != null ? textureView.getClass().getName() : "null") +
            ", sampler=" + (sampler != null ? sampler.getClass().getName() : "null"));

        if (textureView == null) {
            textureBindings.remove(name);
        } else if (textureView instanceof BassaltTextureView) {
            long texturePtr = ((BassaltTextureView) textureView).getNativePtr();
            long samplerPtr = sampler != null && sampler instanceof BassaltSampler
                    ? ((BassaltSampler) sampler).getNativePtr()
                    : 0;
            textureBindings.put(name, new TextureBinding(texturePtr, samplerPtr));
            System.out.println("[Bassalt] bindTexture: Bound texture " + name + " (ptr=" + texturePtr + ", samplerPtr=" + samplerPtr + ")");
        } else {
            System.err.println("[Bassalt] WARNING: textureView is NOT BassaltTextureView! Type: " + textureView.getClass().getName());
        }
    }

    @Override
    public void setUniform(@Nullable String name, GpuBuffer value) {
        checkClosed();
        if (name == null || value == null)
            return;

        if (value instanceof BassaltBuffer) {
            long bufferPtr = ((BassaltBuffer) value).getNativePtr();
            System.out.println("[Bassalt] setUniform: name=" + name + ", size=" + value.size());
            uniformBindings.put(name, new UniformBinding(bufferPtr, 0, value.size()));
        }
    }

    @Override
    public void setUniform(@Nullable String name, GpuBufferSlice value) {
        checkClosed();
        if (name == null || value == null)
            return;

        if (value.buffer() instanceof BassaltBuffer) {
            long bufferPtr = ((BassaltBuffer) value.buffer()).getNativePtr();
            System.out.println("[Bassalt] setUniform (slice): name=" + name + ", offset=" + value.offset() + ", size=" + value.length());
            uniformBindings.put(name, new UniformBinding(bufferPtr, value.offset(), value.length()));
        }
    }

    @Override
    public void enableScissor(int x, int y, int width, int height) {
        checkClosed();
        BassaltDevice.setScissorRect(device.getNativePtr(), nativePassPtr, x, y, width, height);
    }

    @Override
    public void disableScissor() {
        checkClosed();
        // Disable scissor by setting it to a very large rect (effectively disabling
        // clipping)
        // WebGPU doesn't have a "disable scissor" command, so we set it to
        // viewport-sized rect
        // For now, use a large value that covers any reasonable viewport
        BassaltDevice.setScissorRect(device.getNativePtr(), nativePassPtr, 0, 0, 16384, 16384);
    }

    @Override
    public void setVertexBuffer(int slot, @Nullable GpuBuffer vertexBuffer) {
        checkClosed();
        if (vertexBuffer == null || !(vertexBuffer instanceof BassaltBuffer))
            return;

        long bufferPtr = ((BassaltBuffer) vertexBuffer).getNativePtr();
        device.setVertexBuffer(
                device.getNativePtr(),
                nativePassPtr,
                slot,
                bufferPtr,
                0);
    }

    @Override
    public void setIndexBuffer(@Nullable GpuBuffer indexBuffer, VertexFormat.@Nullable IndexType indexType) {
        checkClosed();
        if (indexBuffer == null || !(indexBuffer instanceof BassaltBuffer))
            return;

        long bufferPtr = ((BassaltBuffer) indexBuffer).getNativePtr();
        int type = indexType == VertexFormat.IndexType.INT ? 1 : 0;

        device.setIndexBuffer(
                device.getNativePtr(),
                nativePassPtr,
                bufferPtr,
                type,
                0);
    }

    @Override
    public void drawIndexed(int baseVertex, int firstIndex, int indexCount, int instanceCount) {
        checkClosed();
        
        // Skip draw if no valid pipeline
        if (currentPipelineHandle == 0) {
            return;
        }
        
        // Apply bindings before drawing
        applyBindings();
        
        // Skip draw if bind group creation failed (pipeline expects bindings we can't provide)
        if (!hasValidBindGroup) {
            return;
        }

        device.drawIndexed(
                device.getNativePtr(),
                nativePassPtr,
                indexCount,
                instanceCount,
                firstIndex,
                baseVertex,
                0);
    }

    @Override
    public <T> void drawMultipleIndexed(
            Collection<Draw<T>> draws,
            @Nullable GpuBuffer defaultIndexBuffer,
            VertexFormat.@Nullable IndexType defaultIndexType,
            @Nullable Collection<String> dynamicUniforms,
            T uniformArgument) {
        for (Draw<T> draw : draws) {
            // Draw record has: slot, vertexBuffer, indexBuffer, indexType, firstIndex,
            // indexCount, uniformUploaderConsumer
            // Pipeline should be set separately with setPipeline() before calling this
            // method
            setVertexBuffer(draw.slot(), draw.vertexBuffer());

            GpuBuffer indexBuf = draw.indexBuffer() != null ? draw.indexBuffer() : defaultIndexBuffer;
            VertexFormat.IndexType indexType = draw.indexType() != null ? draw.indexType() : defaultIndexType;

            if (indexBuf != null && indexType != null) {
                setIndexBuffer(indexBuf, indexType);
            }

            if (draw.uniformUploaderConsumer() != null) {
                draw.uniformUploaderConsumer().accept(uniformArgument, this::setUniform);
            }

            drawIndexed(0, draw.firstIndex(), draw.indexCount(), 1);
        }
    }

    @Override
    public void draw(int firstVertex, int vertexCount) {
        checkClosed();
        
        // Skip draw if no valid pipeline
        if (currentPipelineHandle == 0) {
            return;
        }
        
        // Apply bindings before drawing
        applyBindings();
        
        // Skip draw if bind group creation failed
        if (!hasValidBindGroup) {
            return;
        }

        BassaltDevice.draw(
                device.getNativePtr(),
                nativePassPtr,
                vertexCount,
                1, // instanceCount
                firstVertex,
                0 // firstInstance
        );
    }

    @Override
    public void close() {
        if (!closed) {
            device.endRenderPass(device.getNativePtr(), nativePassPtr);
            closed = true;
        }
    }

    // Track if we have a valid bind group for current draw
    private boolean hasValidBindGroup = false;

    /**
     * Apply current bindings as multiple bind groups (wgpu-mc style)
     * Group 0: Textures/samplers
     * Group 1: DynamicTransforms uniform
     * Group 2: Projection uniform
     */
    private void applyBindings() {
        hasValidBindGroup = false;
        
        // Convert binding maps to arrays for JNI call
        String[] textureNames = textureBindings.keySet().toArray(new String[0]);
        long[] textures = new long[textureBindings.size()];
        long[] samplers = new long[textureBindings.size()];

        int i = 0;
        for (TextureBinding binding : textureBindings.values()) {
            textures[i] = binding.texturePtr;
            samplers[i] = binding.samplerPtr;
            i++;
        }

        String[] uniformNames = uniformBindings.keySet().toArray(new String[0]);
        long[] uniforms = new long[uniformBindings.size()];
        long[] uniformOffsets = new long[uniformBindings.size()];
        long[] uniformSizes = new long[uniformBindings.size()];

        i = 0;
        for (UniformBinding binding : uniformBindings.values()) {
            uniforms[i] = binding.bufferPtr;
            uniformOffsets[i] = binding.offset;
            uniformSizes[i] = binding.size;
            i++;
        }

        // Debug: log what we're trying to bind
        if (textureBindings.size() > 0 || uniformBindings.size() > 0) {
            System.out.println("[Bassalt] applyBindings: " + textureBindings.size() + " textures, " + 
                uniformBindings.size() + " uniforms, pipeline=" + currentPipelineHandle);
        }

        // Try multi-bind-group approach first (wgpu-mc style)
        // The native code creates separate bind groups for each group index
        long multiResult = createMultiBindGroups(
                device.getNativePtr(),
                nativePassPtr,
                currentPipelineHandle,
                textureNames,
                textures,
                samplers,
                uniformNames,
                uniforms,
                uniformOffsets,
                uniformSizes);

        if (multiResult != 0) {
            // Multi-bind-group succeeded, native code logged the handles
            // The native code sets up bind groups internally
            hasValidBindGroup = true;
        } else {
            // Fallback: try single bind group approach
            long bindGroupPtr = createBindGroup0(
                    device.getNativePtr(),
                    nativePassPtr,
                    currentPipelineHandle,
                    textureNames,
                    textures,
                    samplers,
                    uniformNames,
                    uniforms);

            if (bindGroupPtr != 0) {
                setBindGroup0(device.getNativePtr(), nativePassPtr, 0, bindGroupPtr);
                hasValidBindGroup = true;
            } else {
                hasValidBindGroup = (currentPipelineHandle != 0);
            }
        }
    }

    private void checkClosed() {
        if (closed) {
            throw new IllegalStateException("Render pass is closed");
        }
    }

    /**
     * Internal class to track texture bindings
     */
    private static class TextureBinding {
        final long texturePtr;
        final long samplerPtr;

        TextureBinding(long texturePtr, long samplerPtr) {
            this.texturePtr = texturePtr;
            this.samplerPtr = samplerPtr;
        }
    }

    /**
     * Internal class to track uniform buffer bindings
     */
    private static class UniformBinding {
        final long bufferPtr;
        final long offset;
        final long size;

        UniformBinding(long bufferPtr, long offset, long size) {
            this.bufferPtr = bufferPtr;
            this.offset = offset;
            this.size = size;
        }
    }
}
