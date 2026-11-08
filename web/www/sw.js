// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// Offline use of the browser editor (once installed, or after one visit):
// the editor's files are kept in the browser's cache. Online, every file
// comes from the site first, so a new version is used as soon as it is
// published; only when that fails (offline) does the kept copy answer.
// Diagrams themselves never pass through here: they are files on the
// user's computer.

const CACHE = "stratagram-editor"
const CORE = [
    "./",
    "app.js",
    "canvas.js",
    "panel.js",
    "stratagram.css",
    "manifest.webmanifest",
    "pkg/stratagram_web.js",
    "pkg/stratagram_web_bg.wasm",
    "pkg/stratagram.svg",
    "icons/icon-192.png",
    "icons/icon-512.png",
]

self.addEventListener("install", (event) => {
    event.waitUntil(
        caches.open(CACHE)
            .then((cache) => cache.addAll(CORE.map((url) => new Request(url, { cache: "reload" }))))
            .then(() => self.skipWaiting()),
    )
})

self.addEventListener("activate", (event) => {
    event.waitUntil(self.clients.claim())
})

self.addEventListener("fetch", (event) => {
    const request = event.request
    if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) return
    event.respondWith(
        fetch(request)
            .then((response) => {
                if (response.ok) {
                    const copy = response.clone()
                    event.waitUntil(caches.open(CACHE).then((cache) => cache.put(request, copy)))
                }
                return response
            })
            .catch(async () => {
                const kept = await caches.match(request, { ignoreSearch: true })
                if (kept) return kept
                // The editor's page for any address in it (e.g. with a query).
                if (request.mode === "navigate") return (await caches.match("./")) || Response.error()
                return Response.error()
            }),
    )
})
