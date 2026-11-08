# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

import json

from django.db import transaction
from django.http import HttpResponseBadRequest, HttpResponseForbidden, JsonResponse
from django.shortcuts import get_object_or_404
from django.template import TemplateDoesNotExist
from django.template.loader import get_template
from django.urls import reverse
from django.middleware.csrf import get_token
from django.templatetags.static import static
from django.utils.translation import gettext_lazy as _
from django.views import View

from dcim.models import Location, Site
from netbox.plugins.utils import get_plugin_config
from netbox.views import generic
from utilities.views import ViewTab, register_model_view

from .data import site_data

try:
    from .editor_build import EDITOR
except ImportError:  # not built (see build.sh)
    EDITOR = None
from .models import SiteDiagram

__all__ = ("LocationDiagramView", "SiteDiagramDataView", "SiteDiagramView", "SiteNetBoxDataView")

PLUGIN = "netbox_stratagram"

# Largest diagram accepted (it holds logos as images).
MAX_DIAGRAM_BYTES = 16 * 1024 * 1024


VIEW = f"{PLUGIN}.view_sitediagram"


class _NotAllowed(Exception):
    """Saved, but outside what the user may add or change: rolled back."""


def _can_save(user, diagram):
    """Whether `user` may save over `diagram` (with the constraints of
    their permissions to change it), or add a site's first one (whether it
    is within their constraints shows once saved, see `_allowed`)."""
    if diagram is None:
        return user.has_perm(f"{PLUGIN}.add_sitediagram")
    return user.has_perm(f"{PLUGIN}.change_sitediagram", diagram)


def _allowed(user, diagram, action):
    """Whether the saved `diagram` is one `user` may `action` ("add",
    "change"), by the constraints of their permissions."""
    return SiteDiagram.objects.restrict(user, action).filter(pk=diagram.pk).exists()


def _visible(user, site):
    """The site's saved diagram as far as `user` may view it: (the
    diagram or None, whether one is saved that they may not view)."""
    saved = SiteDiagram.objects.filter(site=site).first()
    if saved is None:
        return None, False
    if SiteDiagram.objects.restrict(user, "view").filter(pk=saved.pk).exists():
        return saved, False
    return None, True


def _editor_context(request, site, base_template, sheet=None):
    """The editor with `site`'s diagram (saved, or freshly imported from
    NetBox), on the page `base_template`, showing the sheet standing for
    `sheet` ("dcim.location:4") if given."""
    diagram, hidden = _visible(request.user, site)
    # The tab is hidden without the permission, but its address still works.
    if not request.user.has_perm(VIEW):
        denied = "You may not view site diagrams."
    elif hidden:
        # Not shown, nor drawn afresh to be saved over.
        denied = "This site has a diagram, but you may not view it."
    else:
        denied = None
    return {
        "base_template": base_template,
        "denied": denied,
        "editor_css": static(f"netbox_stratagram/editor/{EDITOR}/stratagram.css") if EDITOR else None,
        "editor_version": EDITOR,
        "stratagram": {
            "editorBase": static(f"netbox_stratagram/editor/{EDITOR}/") if EDITOR else None,
            "site": site.name,
            "sheet": sheet,
            "name": f"{site.slug}.stratagram",
            "dataUrl": reverse(f"plugins:{PLUGIN}:site_netbox_data", kwargs={"pk": site.pk}),
            "diagramUrl": reverse(f"plugins:{PLUGIN}:site_diagram_data", kwargs={"pk": site.pk}),
            "canSave": not denied and _can_save(request.user, diagram),
            "csrfToken": get_token(request),
            "saved": diagram.last_updated.isoformat() if diagram and diagram.last_updated else None,
            "options": {
                "kinds": get_plugin_config(PLUGIN, "kinds") or {},
                "all_devices": bool(get_plugin_config(PLUGIN, "all_devices")),
                "type_layers": bool(get_plugin_config(PLUGIN, "type_layers", True)),
                "routing": get_plugin_config(PLUGIN, "routing") or "orthogonal",
                "fronts": list(get_plugin_config(PLUGIN, "fronts") or ["switch"]),
            },
        },
    }


def _page_template(obj):
    """The template of `obj`'s own page, which the Diagram tab extends:
    "dcim/site.html", say, or NetBox's generic object page where the model
    has none of its own (locations from NetBox 4.7 on)."""
    name = f"{obj._meta.app_label}/{obj._meta.model_name}.html"
    try:
        get_template(name)
        return name
    except TemplateDoesNotExist:
        return "generic/object.html"


@register_model_view(Site, "stratagram", path="diagram")
class SiteDiagramView(generic.ObjectView):
    """The site's Diagram tab: the editor, with the saved diagram (or the
    site freshly imported from NetBox)."""

    queryset = Site.objects.all()
    template_name = "netbox_stratagram/diagram.html"
    tab = ViewTab(label=_("Diagram"), permission=VIEW, weight=1500)

    def get_extra_context(self, request, instance):
        return _editor_context(request, instance, _page_template(instance))


@register_model_view(Location, "stratagram", path="diagram")
class LocationDiagramView(generic.ObjectView):
    """A location's Diagram tab: its site's diagram, open on the
    location's sheet."""

    queryset = Location.objects.all()
    template_name = "netbox_stratagram/diagram.html"
    tab = ViewTab(label=_("Diagram"), permission=VIEW, weight=1500)

    def get_extra_context(self, request, instance):
        return _editor_context(request, instance.site, _page_template(instance), f"dcim.location:{instance.pk}")


class SiteNetBoxDataView(View):
    """GET: the site's devices, cabled interfaces and cables (JSON)."""

    def get(self, request, pk):
        site = get_object_or_404(Site.objects.restrict(request.user, "view"), pk=pk)
        return JsonResponse(site_data(site, request.user))


class SiteDiagramDataView(View):
    """GET: the saved diagram ({"diagram": ..., "saved": ...}, 404 if none).
    POST {"diagram": ...}: save it."""

    def get(self, request, pk):
        site = get_object_or_404(Site.objects.restrict(request.user, "view"), pk=pk)
        diagram = get_object_or_404(SiteDiagram.objects.restrict(request.user, "view"), site=site)
        return JsonResponse({"diagram": diagram.data, "saved": diagram.last_updated})

    def post(self, request, pk):
        site = get_object_or_404(Site.objects.restrict(request.user, "view"), pk=pk)
        diagram = SiteDiagram.objects.filter(site=site).first()
        # Not over one they may not view, unseen.
        hidden = diagram is not None and _visible(request.user, site)[1]
        if hidden or not _can_save(request.user, diagram):
            return HttpResponseForbidden("You may not save diagrams.")
        if len(request.body) > MAX_DIAGRAM_BYTES:
            return HttpResponseBadRequest("The diagram is too large.")
        try:
            data = json.loads(request.body)["diagram"]
        except (ValueError, KeyError, TypeError):
            return HttpResponseBadRequest("Expected {\"diagram\": ...}.")
        if not isinstance(data, dict) or not isinstance(data.get("devices", []), list):
            return HttpResponseBadRequest("Not a diagram.")
        action = "add" if diagram is None else "change"
        if diagram is None:
            diagram = SiteDiagram(site=site)
        else:
            # For the change log: what it was before.
            diagram.snapshot()
        diagram.data = data
        diagram.full_clean()
        # Like NetBox's own edit views: saved, then checked against the
        # constraints of the user's permissions, and undone outside them.
        try:
            with transaction.atomic():
                diagram.save()
                if not _allowed(request.user, diagram, action):
                    raise _NotAllowed
        except _NotAllowed:
            return HttpResponseForbidden("You may not save this site's diagram.")
        return JsonResponse({"saved": diagram.last_updated})
