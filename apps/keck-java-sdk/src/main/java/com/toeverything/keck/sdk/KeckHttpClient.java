package com.toeverything.keck.sdk;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;

import java.io.IOException;

import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;

/**
 * Low-level HTTP client wrapper around OkHttp for the Keck API.
 */
public class KeckHttpClient {

    private static final MediaType JSON = MediaType.get("application/json; charset=utf-8");

    private final OkHttpClient httpClient;
    private final KeckConfig config;
    private final Gson gson;

    public KeckHttpClient(KeckConfig config) {
        this.config = config;
        this.httpClient = new OkHttpClient();
        this.gson = new Gson();
    }

    public KeckHttpClient(KeckConfig config, OkHttpClient httpClient) {
        this.config = config;
        this.httpClient = httpClient;
        this.gson = new Gson();
    }

    public KeckConfig getConfig() {
        return config;
    }

    public OkHttpClient getHttpClient() {
        return httpClient;
    }

    public Gson getGson() {
        return gson;
    }

    /**
     * Perform a GET request and return parsed JSON.
     */
    public JsonElement get(String relativePath) {
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .get()
                .build();
        return executeForJson(request);
    }

    /**
     * Perform a GET request. Returns null body on 404; throws on other errors.
     */
    public JsonElement getOrNull(String relativePath) {
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .get()
                .build();
        try (Response response = httpClient.newCall(request).execute()) {
            if (response.code() == 404) {
                return null;
            }
            if (!response.isSuccessful()) {
                throw new KeckException("GET " + relativePath + " failed: " + response.code(), response.code());
            }
            String body = response.body() != null ? response.body().string() : "";
            if (body.isEmpty()) {
                return null;
            }
            return JsonParser.parseString(body);
        } catch (KeckException e) {
            throw e;
        } catch (IOException e) {
            throw new KeckException("GET " + relativePath + " failed", e);
        }
    }

    /**
     * Perform a POST request with a JSON body.
     */
    public JsonElement post(String relativePath, Object bodyObj) {
        String json = gson.toJson(bodyObj);
        RequestBody body = RequestBody.create(json, JSON);
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .post(body)
                .build();
        return executeForJson(request);
    }

    /**
     * Perform a POST with empty body and return the response JSON or null.
     */
    public JsonElement postEmpty(String relativePath) {
        RequestBody body = RequestBody.create("", JSON);
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .post(body)
                .build();
        return executeForJson(request);
    }

    /**
     * Perform a DELETE request.
     */
    public void delete(String relativePath) {
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .delete()
                .build();
        try (Response response = httpClient.newCall(request).execute()) {
            if (!response.isSuccessful() && response.code() != 204) {
                throw new KeckException("DELETE " + relativePath + " failed: " + response.code(), response.code());
            }
        } catch (KeckException e) {
            throw e;
        } catch (IOException e) {
            throw new KeckException("DELETE " + relativePath + " failed", e);
        }
    }

    /**
     * Perform a GET request and return raw response string.
     */
    public String getRaw(String relativePath) {
        Request request = new Request.Builder()
                .url(config.buildUrl(relativePath))
                .get()
                .build();
        try (Response response = httpClient.newCall(request).execute()) {
            if (!response.isSuccessful()) {
                throw new KeckException("GET " + relativePath + " failed: " + response.code(), response.code());
            }
            return response.body() != null ? response.body().string() : "";
        } catch (KeckException e) {
            throw e;
        } catch (IOException e) {
            throw new KeckException("GET " + relativePath + " failed", e);
        }
    }

    private JsonElement executeForJson(Request request) {
        try (Response response = httpClient.newCall(request).execute()) {
            if (!response.isSuccessful()) {
                throw new KeckException(request.method() + " " + request.url() + " failed: " + response.code(),
                        response.code());
            }
            String body = response.body() != null ? response.body().string() : "";
            if (body.isEmpty()) {
                return null;
            }
            return JsonParser.parseString(body);
        } catch (KeckException e) {
            throw e;
        } catch (IOException e) {
            throw new KeckException(request.method() + " " + request.url() + " failed", e);
        }
    }
}
