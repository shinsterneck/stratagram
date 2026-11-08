# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

from django.db import models
from django.urls import reverse
from django.utils.translation import gettext_lazy as _

from netbox.models import ChangeLoggedModel

__all__ = ("SiteDiagram",)


class SiteDiagram(ChangeLoggedModel):
    """The diagram of a site as arranged in the editor (a .stratagram file)."""

    site = models.OneToOneField(
        to="dcim.Site",
        on_delete=models.CASCADE,
        related_name="stratagram_diagram",
        verbose_name=_("site"),
    )
    data = models.JSONField(verbose_name=_("diagram"))

    class Meta:
        ordering = ("site",)
        verbose_name = _("site diagram")
        verbose_name_plural = _("site diagrams")

    def __str__(self):
        return f"Diagram of {self.site}"

    def get_absolute_url(self):
        return reverse("dcim:site_stratagram", kwargs={"pk": self.site_id})
