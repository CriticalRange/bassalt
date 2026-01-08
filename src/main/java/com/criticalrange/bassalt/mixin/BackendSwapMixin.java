package com.criticalrange.bassalt.mixin;

import com.criticalrange.bassalt.backend.BassaltBackend;
import com.mojang.blaze3d.opengl.GlBackend;
import com.mojang.blaze3d.platform.Window;
import com.mojang.blaze3d.systems.GpuBackend;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Mixin to inject BassaltBackend into the backend array
 *
 * This mixin modifies the backend array used by Minecraft to include
 * BassaltRenderer as the first option, falling back to OpenGL if it fails.
 */
@Mixin(Window.class)
public class BackendSwapMixin {

    private static final Logger LOGGER = LogManager.getLogger("Bassalt");

    /**
     * Modify the backends array parameter in Window constructor
     * Uses @ModifyVariable at HEAD to modify the parameter before the constructor body runs
     *
     * Based on: https://wiki.fabricmc.net/tutorial:mixin_examples
     */
    @ModifyVariable(
        method = "<init>",
        at = @At("HEAD"),
        argsOnly = true,
        require = 1
    )
    private static GpuBackend[] bassalt$modifyBackendsArray(GpuBackend[] original) {
        LOGGER.debug("Modifying backends array in Window constructor");
        LOGGER.debug("bassalt.enabled property: {}", System.getProperty("bassalt.enabled"));
        if (Boolean.getBoolean("bassalt.enabled")) {
            LOGGER.debug("Injecting Bassalt backend into array");
            return new GpuBackend[]{new BassaltBackend(), new GlBackend()};
        }
        return original;
    }
}
