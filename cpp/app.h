// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// Application-wide settings Qt only offers in C++.
#pragma once

#include <QtCore/QString>
#include <QtGui/QGuiApplication>
#include <QtGui/QStyleHints>

inline void setup_app()
{
    // Matches packaging/stratagram.desktop, so the desktop (and its file
    // dialogs, through the portal) knows which app this is.
    QGuiApplication::setDesktopFileName(QStringLiteral("stratagram"));
    // The interface is designed light (dark text on light panels); keep it
    // so when the desktop prefers dark.
    QGuiApplication::styleHints()->setColorScheme(Qt::ColorScheme::Light);
}
