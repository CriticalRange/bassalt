package com.criticalrange.bassalt.backend;

import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Bridge class for native code to log through Java's Log4j system
 * This ensures native logs appear in Minecraft's log files
 */
public class BassaltLogger {
    private static final Logger LOGGER = LogManager.getLogger("BassaltNative");
    
    // Log levels
    public static final int LEVEL_TRACE = 0;
    public static final int LEVEL_DEBUG = 1;
    public static final int LEVEL_INFO = 2;
    public static final int LEVEL_WARN = 3;
    public static final int LEVEL_ERROR = 4;
    public static final int LEVEL_FATAL = 5;
    
    /**
     * Initialize the native logging bridge
     * This should be called once during backend initialization
     */
    public static native void initNativeLogger();
    
    /**
     * Called from native code to log a message
     * @param level Log level (use LEVEL_* constants)
     * @param message Log message
     */
    public static void log(int level, String message) {
        switch (level) {
            case LEVEL_TRACE:
                LOGGER.trace(message);
                break;
            case LEVEL_DEBUG:
                LOGGER.debug(message);
                break;
            case LEVEL_INFO:
                LOGGER.info(message);
                break;
            case LEVEL_WARN:
                LOGGER.warn(message);
                break;
            case LEVEL_ERROR:
                LOGGER.error(message);
                break;
            case LEVEL_FATAL:
                LOGGER.fatal(message);
                break;
            default:
                LOGGER.info(message);
                break;
        }
    }
    
    /**
     * Called from native code to log a message with exception
     * @param level Log level (use LEVEL_* constants)
     * @param message Log message
     * @param exception Exception message
     */
    public static void logWithException(int level, String message, String exception) {
        switch (level) {
            case LEVEL_TRACE:
                LOGGER.trace(message, new RuntimeException(exception));
                break;
            case LEVEL_DEBUG:
                LOGGER.debug(message, new RuntimeException(exception));
                break;
            case LEVEL_INFO:
                LOGGER.info(message, new RuntimeException(exception));
                break;
            case LEVEL_WARN:
                LOGGER.warn(message, new RuntimeException(exception));
                break;
            case LEVEL_ERROR:
                LOGGER.error(message, new RuntimeException(exception));
                break;
            case LEVEL_FATAL:
                LOGGER.fatal(message, new RuntimeException(exception));
                break;
            default:
                LOGGER.info(message, new RuntimeException(exception));
                break;
        }
    }
}
