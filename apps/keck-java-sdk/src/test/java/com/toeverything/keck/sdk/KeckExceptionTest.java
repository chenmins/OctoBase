package com.toeverything.keck.sdk;

import org.junit.Test;

import static org.junit.Assert.*;

public class KeckExceptionTest {

    @Test
    public void testMessageOnly() {
        KeckException ex = new KeckException("test error");
        assertEquals("test error", ex.getMessage());
        assertEquals(-1, ex.getStatusCode());
    }

    @Test
    public void testMessageAndStatusCode() {
        KeckException ex = new KeckException("not found", 404);
        assertEquals("not found", ex.getMessage());
        assertEquals(404, ex.getStatusCode());
    }

    @Test
    public void testMessageAndCause() {
        Exception cause = new RuntimeException("root cause");
        KeckException ex = new KeckException("wrapper", cause);
        assertEquals("wrapper", ex.getMessage());
        assertEquals(cause, ex.getCause());
        assertEquals(-1, ex.getStatusCode());
    }
}
