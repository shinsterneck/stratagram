// SPDX-License-Identifier: Apache-2.0
import en from "./en.mjs"
import coreMessages from "./core-messages.mjs"
import zhCN from "./zh-CN.mjs"

const catalogs = { en, "zh-CN": zhCN }
export const languages = {}
for (const id of Object.keys(catalogs)) languages[id] = catalogs[id]["language.name"]

export function matchLanguage(tags) {
    for (const tag of tags) {
        const normalized = tag.toLowerCase().replace(/_/g, "-")
        const ids = Object.keys(languages)
        const match = ids.find(id => id.toLowerCase() === normalized)
            || ids.find(id => id.split("-")[0] === normalized.split("-")[0])
        if (match) return match
    }
    return "en"
}

// Preserve the message key when a host or an export operation rejects a promise.
export class MessageError extends Error {
    constructor(key) {
        super(en[key] ?? key)
        this.key = key
    }
}

export function translate(language, key, values) {
    const catalog = catalogs[language] || en
    const message = catalog[key] ?? en[key] ?? key
    if (!values) return message
    return message.replace(/\{(\w+)\}/g, (token, name) => {
        const value = values[name]
        if (value instanceof MessageError) return translate(language, value.key)
        if (value instanceof Error) return value.message
        return value ?? token
    })
}

// Older WASM and native controllers both return fully formatted English.
// Index fixed messages and compile templates once for the rendering boundary.
const coreKeys = new Map(Object.entries(coreMessages).map(([key, text]) => [text, key]))
const patterns = Object.entries(coreMessages).filter(([, text]) => text.includes("{")).map(([key, source]) => {
    const names = []
    const pattern = source.split(/(\{\w+\})/).map(part => {
        if (part.startsWith("{")) {
            const name = part.slice(1, -1)
            names.push(name)
            if (["count", "layers", "devices", "links"].includes(name)) return "(\\d+)"
            return name === "changes" || name === "locations" ? "(.*)" : "(.+)"
        }
        return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
    }).join("")
    return { key, names, pattern: new RegExp("^" + pattern + "$") }
})

function translateStatusValue(language, value) {
    const key = coreKeys.get(value)
    return key ? translate(language, key) : value
}

// NetBox quotes layer names as JSON strings, including names with commas.
function netboxParts(text) {
    return text.match(/(?:"(?:\\.|[^"\\])*"|[^",]|,(?! ))+/g)?.map(part => part.trim()) || []
}

export function translateStatus(language, message) {
    if (!message) return message
    const key = coreKeys.get(message)
    if (key) return translate(language, key)
    if (/^(From NetBox:|Up to date with NetBox\.)/.test(message)) {
        const suffix = / (\d+ (?:new )?location sheets? (?:added|renamed)\.)$/.exec(message)
        if (suffix) return translateStatus(language, message.slice(0, suffix.index)) + " " + translateStatus(language, suffix[1])
    }
    for (const { key, names, pattern } of patterns) {
        const match = pattern.exec(message)
        if (!match) continue
        const values = {}
        names.forEach((key, i) => { values[key] = match[i + 1] })
        if (key === "netbox.gone") {
            try { values.name = JSON.parse('"' + values.name + '"') } catch (error) { /* older controller */ }
            if (values.name === "Gone from NetBox") values.name = translate(language, "netbox.goneLayer")
        }
        if (values.error) values.error = translateStatusValue(language, values.error)
        if (values.layout) values.layout = translateStatusValue(language, values.layout)
        if (values.locations) values.locations = translateStatus(language, values.locations)
        if (values.changes) {
            const fromNetbox = key === "netbox.changes"
            const parts = fromNetbox ? netboxParts(values.changes) : values.changes.split("; ")
            values.changes = parts.map(part => translateStatus(language, part)).join(translate(language, fromNetbox ? "list.separator" : "list.detailSeparator"))
        }
        return translate(language, key, values)
    }
    return message
}

// The first line names the endpoints. The last starts with a built-in link
// type and may end in a user-defined label.
export function translateTip(language, lines) {
    return lines.map((line, i) => {
        if (i === 0) return line
        if (i < lines.length - 1) return translateStatus(language, line)
        const divider = line.indexOf(" · ")
        return divider < 0 ? translateStatusValue(language, line)
            : translateStatusValue(language, line.slice(0, divider)) + line.slice(divider)
    })
}
