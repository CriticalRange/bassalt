package com.criticalrange.bassalt.mixin;

import com.criticalrange.bassalt.backend.BassaltBackend;
import com.mojang.blaze3d.opengl.GlBackend;
import com.mojang.blaze3d.systems.GpuBackend;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * Mixin to inject BassaltBackend into the backend array
 *
 * This mixin modifies the backend array used by Minecraft to include
 * BassaltRenderer as the first option, falling back to OpenGL if it fails.
 */
@Mixin(Minecraft.class)
public class BackendSwapMixin {

    /**
     * Modify the GpuBackend array to include Bassalt as the first option
     * Original: new GpuBackend[]{new GlBackend()}
     * Modified: new GpuBackend[]{new BassaltBackend(), new GlBackend()}
     */
    @ModifyArg(
        method = "<init>",
        at = @At(
            value = "INVOKE",
            target = "Lcom/mojang/blaze3d/platform/Window;<init>*",
            shift = At.Shift.AFTER
        ),
        index = 0  // First argument to the Window constructor (the backend array)
    )
    private static GpuBackend[] bassalt$addBassaltBackend(GpuBackend[] original) {
        // Only inject Bassalt if enabled via system property
        if (Boolean.getBoolean("bassalt.enabled")) {
            System.out.println("[Bassalt] Injecting Bassalt backend into backend array");
            return new GpuBackend[]{
                new BassaltBackend(),  // Try Bassalt first
                new GlBackend()        // Fallback to OpenGL
            };
        }
        return original;
    }
}
