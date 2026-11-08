// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// Clipboard access for the Rust side; cxx-qt-lib has no QClipboard binding.
#pragma once

#include <QtCore/QMimeData>
#include <QtCore/QString>
#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>

// Our own format, next to the plain text, so pasting back into stratagram
// keeps everything (ports, styles, connection points).
inline const char* stratagram_mime() { return "application/x-stratagram+json"; }

// `text` (draw.io XML) as plain text only, with no HTML copy, so apps like
// draw.io read it as data; `native` under our own type.
inline void set_clipboard(const QString& text, const QString& native)
{
    auto* data = new QMimeData;
    data->setText(text);
    data->setData(stratagram_mime(), native.toUtf8());
    QGuiApplication::clipboard()->setMimeData(data);
}

// Our own data on the clipboard, or empty.
inline QString clipboard_native()
{
    const QMimeData* data = QGuiApplication::clipboard()->mimeData();
    return data && data->hasFormat(stratagram_mime()) ? QString::fromUtf8(data->data(stratagram_mime())) : QString();
}

inline QString clipboard_text()
{
    return QGuiApplication::clipboard()->text();
}
