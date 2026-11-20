// SPDX-License-Identifier: Apache-2.0
import assert from "node:assert/strict"
import test from "node:test"
import { matchLanguage, translate, translateStatus, translateTip } from "../www/locales/messages.mjs"
import en from "../www/locales/en.mjs"
import coreMessages from "../www/locales/core-messages.mjs"
import zhCN from "../www/locales/zh-CN.mjs"

test("pick the first supported language and fall back to English", () => {
    assert.equal(matchLanguage(["de-DE", "zh-Hans-CN", "en-US"]), "zh-CN")
    assert.equal(matchLanguage(["en-US", "zh-CN"]), "en")
    assert.equal(matchLanguage(["de-DE"]), "en")
    assert.equal(translate("fr", "file.save"), "Save")
    assert.equal(translate("zh-CN", "file.save"), "保存")
})

test("catalogs have identical keys and retain named parameters", () => {
    assert.deepEqual(Object.keys(zhCN).sort(), Object.keys(en).sort())
    const parameters = text => [...new Set(text.match(/\{\w+\}/g) || [])].sort()
    for (const [key, translation] of Object.entries(zhCN)) {
        assert.deepEqual(parameters(translation), parameters(en[key]), key)
    }
    for (const [key, format] of Object.entries(coreMessages)) {
        assert.ok(Object.hasOwn(en, key), key)
        assert.deepEqual(parameters(format), parameters(en[key]), key)
    }
})

test("names inside status messages and cable labels stay verbatim", () => {
    const name = "Save {name} $&"
    assert.equal(translateStatus("zh-CN", "Saved " + name), "已保存 " + name)
    assert.equal(translateStatus("en", "Saved " + name), "Saved " + name)
    const lines = ["Router  ⟷  Save", "Through hidden devices (2 cables)", "IPsec tunnel · Delete"]
    assert.deepEqual(translateTip("zh-CN", lines), [lines[0], "经过隐藏设备（2 条线缆）", "IPsec 隧道 · Delete"])
})
