package com.toeverything.keck.sdk;

/**
 * Exception thrown when a Keck API call fails.
 */
public class KeckException extends RuntimeException {

    private final int statusCode;

    public KeckException(String message) {
        super(message);
        this.statusCode = -1;
    }

    public KeckException(String message, int statusCode) {
        super(message);
        this.statusCode = statusCode;
    }

    public KeckException(String message, Throwable cause) {
        super(message, cause);
        this.statusCode = -1;
    }

    /**
     * @return HTTP status code, or -1 if not applicable
     */
    public int getStatusCode() {
        return statusCode;
    }
}
