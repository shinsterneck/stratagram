# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

import django.db.models.deletion
from django.db import migrations, models


class Migration(migrations.Migration):
    initial = True

    dependencies = [
        ("dcim", "0002_squashed"),
    ]

    operations = [
        migrations.CreateModel(
            name="SiteDiagram",
            fields=[
                ("id", models.BigAutoField(auto_created=True, primary_key=True, serialize=False)),
                ("created", models.DateTimeField(auto_now_add=True, blank=True, null=True, verbose_name="created")),
                ("last_updated", models.DateTimeField(auto_now=True, blank=True, null=True, verbose_name="last updated")),
                ("data", models.JSONField(verbose_name="diagram")),
                (
                    "site",
                    models.OneToOneField(
                        on_delete=django.db.models.deletion.CASCADE,
                        related_name="stratagram_diagram",
                        to="dcim.site",
                        verbose_name="site",
                    ),
                ),
            ],
            options={
                "verbose_name": "site diagram",
                "verbose_name_plural": "site diagrams",
                "ordering": ("site",),
            },
        ),
    ]
