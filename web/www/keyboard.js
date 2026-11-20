// SPDX-License-Identifier: Apache-2.0

// Some browsers end composition just before the final keydown and report 229.
export function composing(event) {
    return event.isComposing || event.keyCode === 229
}
