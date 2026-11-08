// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// PNG export: the SVG export rendered by Qt's SVG renderer, so the picture
// matches the SVG file exactly.
#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QCoreApplication>
#include <QtCore/QString>
#include <QtGui/QGuiApplication>
#include <QtGui/QImage>
#include <QtGui/QPainter>
#include <QtSvg/QSvgRenderer>
#include <algorithm>
#include <cmath>

// Largest side of the picture, in pixels; a larger scale is reduced to fit.
inline constexpr double png_max_side = 16384.0;

// Render `svg` at `scale` (1 = one pixel per diagram unit) into the PNG
// file `path`. Returns an error message, or nothing on success.
inline QString svg_to_png(const QString& svg, const QString& path, double scale)
{
    // Text needs a GUI application; from the command line there is none
    // yet, so make one that draws off screen.
    if (!QCoreApplication::instance()) {
        qputenv("QT_QPA_PLATFORM", "offscreen");
        static int argc = 1;
        static char name[] = "stratagram";
        static char* argv[] = { name, nullptr };
        new QGuiApplication(argc, argv);
    }
    QSvgRenderer renderer(svg.toUtf8());
    if (!renderer.isValid())
        return QStringLiteral("the diagram could not be rendered");
    const QSizeF base = renderer.viewBoxF().size();
    if (base.isEmpty())
        return QStringLiteral("the diagram is empty");
    scale = std::min(scale, png_max_side / std::max(base.width(), base.height()));
    QImage image(QSize(int(std::ceil(base.width() * scale)), int(std::ceil(base.height() * scale))), QImage::Format_ARGB32);
    if (image.isNull())
        return QStringLiteral("the picture is too large");
    image.fill(Qt::white);
    QPainter painter(&image);
    painter.setRenderHints(QPainter::Antialiasing | QPainter::TextAntialiasing | QPainter::SmoothPixmapTransform);
    renderer.render(&painter);
    painter.end();
    if (!image.save(path, "PNG"))
        return QStringLiteral("cannot write ") + path;
    return QString();
}
