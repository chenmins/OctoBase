package com.toeverything.keck.sdk;

import java.io.IOException;
import java.io.InputStream;
import java.util.Properties;

/**
 * Configuration for the Keck SDK.
 * Loads settings from keck-sdk.properties on the classpath,
 * or can be constructed programmatically.
 */
public class KeckConfig {

    private static final String DEFAULT_PROPERTIES = "keck-sdk.properties";
    private static final String DEFAULT_ENDPOINT = "http://localhost:3000/";

    private final String endpoint;

    /**
     * Create a config with the given endpoint.
     *
     * @param endpoint base URL of the Keck API (e.g. "http://localhost:3000/")
     */
    public KeckConfig(String endpoint) {
        // Ensure endpoint ends with /
        this.endpoint = endpoint.endsWith("/") ? endpoint : endpoint + "/";
    }

    /**
     * Load config from the default classpath properties file.
     *
     * @return KeckConfig loaded from keck-sdk.properties
     */
    public static KeckConfig loadDefault() {
        Properties props = new Properties();
        try (InputStream is = KeckConfig.class.getClassLoader().getResourceAsStream(DEFAULT_PROPERTIES)) {
            if (is != null) {
                props.load(is);
            }
        } catch (IOException e) {
            // Ignore, use defaults
        }
        String ep = props.getProperty("keck.endpoint", DEFAULT_ENDPOINT);
        return new KeckConfig(ep);
    }

    /**
     * @return the base API endpoint (always ends with /)
     */
    public String getEndpoint() {
        return endpoint;
    }

    /**
     * Build a full API URL from a relative path.
     *
     * @param relativePath relative path (without leading /)
     * @return full URL string
     */
    public String buildUrl(String relativePath) {
        String path = relativePath.startsWith("/") ? relativePath.substring(1) : relativePath;
        return endpoint + path;
    }
}
