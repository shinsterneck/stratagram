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
    // The app's ID, as in packaging/app.stratagram.Stratagram.desktop, so
    // the desktop shows its icon for the window and knows, for its file
    // dialogs through the portal, which app this is.
    QGuiApplication::setDesktopFileName(QStringLiteral("app.stratagram.Stratagram"));
    // The interface is designed light (dark text on light panels); keep it
    // so when the desktop prefers dark.
    QGuiApplication::styleHints()->setColorScheme(Qt::ColorScheme::Light);
}
