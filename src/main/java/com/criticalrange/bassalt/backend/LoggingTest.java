package com.criticalrange.bassalt.backend;

import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Test class to verify native logging integration
 */
public class LoggingTest {
    private static final Logger LOGGER = LogManager.getLogger("BassaltTest");
    
    public static void testLogging() {
        LOGGER.info("=== Testing Native Logging Bridge ===");
        
        // Test Java logging directly
        LOGGER.info("Java: Direct Log4j logging test");
        
        // Test the bridge logging
        BassaltLogger.log(BassaltLogger.LEVEL_INFO, "Bridge: Test message through BassaltLogger");
        BassaltLogger.log(BassaltLogger.LEVEL_DEBUG, "Bridge: Debug message through BassaltLogger");
        BassaltLogger.log(BassaltLogger.LEVEL_WARN, "Bridge: Warning message through BassaltLogger");
        BassaltLogger.log(BassaltLogger.LEVEL_ERROR, "Bridge: Error message through BassaltLogger");
        
        // Test with exception
        BassaltLogger.logWithException(BassaltLogger.LEVEL_ERROR, 
            "Bridge: Error with exception", "Test exception details");
        
        LOGGER.info("=== Native Logging Bridge Test Complete ===");
    }
}
