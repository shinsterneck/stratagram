# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

from django.urls import path

from . import views

urlpatterns = [
    path("sites/<int:pk>/netbox-data/", views.SiteNetBoxDataView.as_view(), name="site_netbox_data"),
    path("sites/<int:pk>/diagram/", views.SiteDiagramDataView.as_view(), name="site_diagram_data"),
]
