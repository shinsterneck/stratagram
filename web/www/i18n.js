// SPDX-License-Identifier: Apache-2.0
import { languages, matchLanguage, translate } from "./locales/messages.mjs"
export { languages }
export const languageEvents = new EventTarget()
export let language = preferredLanguage()

function preferredLanguage() {
    try {
        const saved = localStorage.getItem("stratagram.language")
        if (Object.hasOwn(languages, saved)) return saved
    } catch { /* private browsing */ }
    return matchLanguage(navigator.languages || [navigator.language])
}

export function setLanguage(id) {
    if (!Object.hasOwn(languages, id)) return
    language = id
    try { localStorage.setItem("stratagram.language", id) } catch { /* session only */ }
    languageEvents.dispatchEvent(new Event("change"))
}

export function tr(key, values) {
    return translate(language, key, values)
}
