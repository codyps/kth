"""Interactive views of the same timing estimates used by the static exports."""

import plotly.graph_objects as go
from plotly.offline import get_plotlyjs
from plotly.subplots import make_subplots


LABELS = {
    "kth": "kth (generic)", "kth-u8": "kth (u8 histogram)",
    "std": "std::select_nth_unstable", "sort": "std::sort (full sort)",
    "sort_unstable": "std::sort_unstable (full sort)",
}
SYMBOLS = dict(zip(
    ("kth", "kth-u8", "std", "order-stat", "pdqselect", "adqselect", "tpt-simd-sort",
     "turboselect", "sort", "sort_unstable", "medians"),
    ("circle", "diamond", "square", "triangle-up", "triangle-down", "cross", "x",
     "star", "circle-open", "square-open", "hexagon"),
))
DASHES = {"std": "dashdot", "sort": "dash", "sort_unstable": "dot"}


def write_assets(output):
    # One local bundle for all charts; downloaded reports also work offline.
    (output / "plotly.min.js").write_text(get_plotlyjs(), encoding="utf-8")


def chart_html(figure, name):
    return figure.to_html(
        full_html=False, include_plotlyjs=False, div_id=f"chart-{name}",
        config={"responsive": True, "scrollZoom": True, "displaylogo": False,
                "toImageButtonOptions": {"format": "svg", "filename": name}},
        post_script="""(() => {
          const chart = document.getElementById('{plot_id}');
          const narrow = window.matchMedia('(max-width: 700px)');
          const originalTitle = chart.layout.title.text;
          function fitLegend() {
            const top = narrow.matches ? 210 : 145;
            const bottom = narrow.matches ? 270 : 100;
            const plotHeight = chart.layout.height - top - bottom;
            const highlight = HIGHLIGHT_MENU;
            const changes = {
              'legend.orientation': narrow.matches ? 'h' : 'v',
              'legend.x': narrow.matches ? 0 : 1.02,
              'legend.y': narrow.matches ? -70 / plotHeight : 1,
              'legend.xanchor': 'left', 'legend.yanchor': 'top',
              'margin.r': narrow.matches ? 25 : 295,
              'margin.l': narrow.matches ? (DEFAULT_LEFT_MARGIN === 100 ? 70 : 170) : DEFAULT_LEFT_MARGIN,
              'margin.b': bottom, 'margin.t': top,
              'updatemenus[0].y': 1 + 35 / plotHeight,
            };
            const parts = originalTitle.split(' · ');
            changes['title.text'] = narrow.matches && parts.length > 2 ?
              parts.slice(0, 2).join(' · ') + '<br>' + parts.slice(2).join(' · ') : originalTitle;
            changes['title.font.size'] = narrow.matches ? 15 : 18;
            chart.querySelector('.modebar').style.top = narrow.matches ? '44px' : '0px';
            if (highlight === 2) changes['updatemenus[1].y'] = 1 + 85 / plotHeight;
            changes[`updatemenus[${highlight}].x`] = narrow.matches || highlight === 1 ? 0 : 1;
            changes[`updatemenus[${highlight}].xanchor`] = narrow.matches || highlight === 1 ? 'left' : 'right';
            changes[`updatemenus[${highlight}].y`] = 1 + (narrow.matches && highlight === 2 ? 135 : 85) / plotHeight;
            Plotly.relayout(chart, changes);
          }
          narrow.addEventListener('change', fitLegend);
          fitLegend();
        })();""".replace("DEFAULT_LEFT_MARGIN", str(figure.layout.margin.l))
           .replace("HIGHLIGHT_MENU", str(len(figure.layout.updatemenus) - 1)),
    )


def label(name):
    return LABELS.get(name, name)


def style(figure, title, height):
    figure.update_layout(
        title={"text": title, "x": 0.04, "y": 0.98, "yanchor": "top"}, height=height, autosize=True,
        paper_bgcolor="#f6f8fb", plot_bgcolor="white",
        font={"family": "Arial, sans-serif", "color": "#172941"},
        margin={"l": 100, "r": 295, "t": 145, "b": 100},
        legend={"orientation": "v", "x": 1.02, "y": 1, "xanchor": "left", "yanchor": "top",
                "groupclick": "togglegroup", "itemsizing": "constant", "title": {"text": "Library / API"}},
        hoverlabel={"namelength": -1}, hovermode="closest", dragmode="zoom",
    )
    figure.update_xaxes(gridcolor="#dce3ed", zeroline=False, automargin=True)
    figure.update_yaxes(gridcolor="#dce3ed", zeroline=False, automargin=True)


def scale_buttons(figure, xaxes, yaxes=(), linear=True):
    def changes(kind):
        return {key: value for axis in (*xaxes, *yaxes)
                for key, value in ((f"{axis}.type", kind), (f"{axis}.autorange", True))}

    buttons = [
        {"label": "Log axes" if yaxes else "Log time", "method": "relayout", "args": [changes("log")]},
        {"label": "Linear axes" if yaxes else "Linear time", "method": "relayout", "args": [changes("linear")]},
    ]
    active = 1 if linear else 0
    if yaxes:
        ticks = sorted({n for trace in figure.data for n in trace.x})
        for button in buttons:
            button["args"][0].update({"xaxis.tickvals": ticks,
                                      "xaxis.title.text": "Number of elements n"})
        categories = list(map(str, ticks))
        evenly_spaced = {"xaxis.type": "category", "xaxis.categoryorder": "array",
                         "xaxis.categoryarray": categories, "xaxis.tickvals": categories,
                         "xaxis.autorange": True,
                         "xaxis.title.text": "Measured sizes (even spacing)<br>labels show actual n"}
        buttons.append({"label": "Even sample spacing", "method": "relayout", "args": [evenly_spaced]})
        # Dispatch-boundary neighbors cannot be read separately on a log axis.
        # Keep the actual n in the data, hover, ratios and CSV; only spacing changes.
        if not linear and any(b / a < 1.02 for a, b in zip(ticks, ticks[1:])):
            active = 2
            figure.update_layout(**evenly_spaced)
    figure.update_layout(updatemenus=[{
        "type": "buttons", "direction": "right", "x": 1, "xanchor": "right", "y": 1.06,
        "showactive": True, "active": active, "buttons": buttons,
    }])


def scaling_figure(rows, title, colors, short=False, linear=False):
    figure = go.Figure()
    baseline = {r["n"]: r["mean_ns"] for r in rows if r["algorithm"] == "kth"}
    series = []
    for name, color in colors.items():
        points = sorted((r for r in rows if r["algorithm"] == name), key=lambda r: r["n"])
        if not points:
            continue
        x = [r["n"] for r in points]
        series.extend((points, field) for field in ("lower_ns", "upper_ns", "mean_ns"))
        # Keep both bounds and the mean in one legend group, including when isolated.
        figure.add_trace(go.Scatter(
            x=x, y=[r["lower_ns"] / 1000 for r in points], mode="lines",
            line={"width": 0}, legendgroup=name, showlegend=False, hoverinfo="skip",
        ))
        rgb = tuple(int(color[i:i + 2], 16) for i in (1, 3, 5))
        figure.add_trace(go.Scatter(
            x=x, y=[r["upper_ns"] / 1000 for r in points], mode="lines",
            line={"width": 0}, fill="tonexty", fillcolor=f"rgba{(*rgb, 0.13)}",
            legendgroup=name, showlegend=False, hoverinfo="skip",
        ))
        figure.add_trace(go.Scatter(
            x=x, y=[r["mean_ns"] / 1000 for r in points], mode="lines+markers",
            name=label(name), legendgroup=name, showlegend=True,
            line={"color": color, "width": 3 if name == "kth" else 2,
                  "dash": DASHES.get(name, "solid")},
            marker={"symbol": SYMBOLS.get(name, "circle"), "size": 8,
                    "line": {"color": color, "width": 1}},
            customdata=[[r["lower_ns"] / 1000, r["upper_ns"] / 1000,
                         r["sample_count"],
                         r["mean_ns"] / baseline[r["n"]] if r["n"] in baseline else None,
                         r["mean_ns"] / 1000] for r in points],
            hovertemplate=("<b>%{fullData.name}</b><br>n = %{x:,}<br>Mean: %{y:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
                           "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples<br>"
                           "%{customdata[3]:.3f}× kth mean time<extra></extra>"),
        ))
    style(figure, title + (" · SHORT RUN" if short else ""), 740)
    ticks = sorted({r["n"] for r in rows})
    figure.update_xaxes(title_text="Number of elements n", type="linear" if linear else "log",
                        tickmode="array", tickvals=ticks, ticktext=[f"{n:,}" for n in ticks])
    figure.update_yaxes(title_text="Mean time per operation (µs)<br>lower is faster",
                        type="linear" if linear else "log", minorloglabels="none")
    scale_buttons(figure, ("xaxis",), ("yaxis",), linear=linear)
    multiplier_buttons(figure, rows, series, colors)
    highlight_buttons(figure)
    return figure


def highlight_buttons(figure):
    """Keep all series for context while making a chosen library easy to follow."""
    groups = list(dict.fromkeys(t.legendgroup for t in figure.data if t.showlegend))
    buttons = [{"label": "Highlight: all libraries", "method": "restyle",
                "args": [{"opacity": [1] * len(figure.data)}]}]
    for name in groups:
        buttons.append({"label": "Highlight: " + label(name), "method": "restyle",
                        "args": [{"opacity": [1 if t.legendgroup == name else 0.12 for t in figure.data]}]})
    figure.update_layout(updatemenus=[*figure.layout.updatemenus, {
        "type": "dropdown", "x": 1, "xanchor": "right", "y": 1.16,
        "active": 0, "buttons": buttons,
    }])


def multiplier_buttons(figure, rows, series, colors):
    """Normalize every trace against a measured mean at exactly the same n.

    Bands scale the numerator's CI by a fixed baseline mean; they are not ratio CIs.
    Missing baseline measurements create gaps rather than interpolated comparisons.
    """
    absolute_hover = (
        "<b>%{fullData.name}</b><br>n = %{x:,}<br>Mean: %{y:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
        "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples<br>"
        "%{customdata[3]:.3f}× kth mean time<extra></extra>"
    )
    buttons = [{
        "label": "Time (µs)", "method": "update", "args": [
            {"y": [[r[field] / 1000 for r in points] for points, field in series],
             "hovertemplate": [absolute_hover if field == "mean_ns" else None for _, field in series]},
            {"yaxis.title.text": "Mean time per operation (µs)<br>lower is faster",
             "yaxis.autorange": True, "shapes": []},
        ],
    }]
    for name in colors:
        baseline = {r["n"]: r["mean_ns"] for r in rows if r["algorithm"] == name}
        if not baseline:
            continue
        hover = (
            f"<b>%{{fullData.name}}</b><br>n = %{{x:,}}<br>Relative time: %{{y:.3f}}× {label(name)}<br>"
            "Mean: %{customdata[4]:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
            "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples"
            "<extra></extra>"
        )
        buttons.append({
            "label": f"Time / {label(name)}", "method": "update", "args": [
                {"y": [[r[field] / baseline[r["n"]] if r["n"] in baseline else None
                        for r in points] for points, field in series],
                 "hovertemplate": [hover if field == "mean_ns" else None for _, field in series]},
                {"yaxis.title.text": f"Time / {name} at the same n (×)<br>lower is faster",
                 "yaxis.autorange": True, "shapes": [{
                     "type": "line", "xref": "paper", "x0": 0, "x1": 1,
                     "yref": "y", "y0": 1, "y1": 1,
                     "line": {"color": "#617489", "width": 1, "dash": "dot"},
                 }] + ([{
                     "type": "line", "xref": "paper", "x0": 0, "x1": 1,
                     "yref": "y", "y0": 0.8, "y1": 0.8,
                     "line": {"color": "#117733", "width": 2, "dash": "dash"},
                     "label": {"text": "20% less time than std", "textposition": "end"},
                 }] if name == "std" else [])},
            ],
        })
    figure.update_layout(updatemenus=[*figure.layout.updatemenus, {
        "type": "dropdown", "x": 0, "xanchor": "left", "y": 1.16,
        "active": 0, "buttons": buttons,
    }])


def fixed_figure(rows, title, cases, colors, short=False):
    figure = make_subplots(rows=len(cases), cols=1, subplot_titles=list(cases.values()),
                           vertical_spacing=0.14 if len(cases) > 1 else 0)
    for index, case in enumerate(cases, 1):
        points = sorted((r for r in rows if r["case"] == case), key=lambda r: r["mean_ns"])
        baseline = next(r["mean_ns"] for r in points if r["algorithm"] == "kth")
        for row in points:
            name = row["algorithm"]
            mean = row["mean_ns"] / 1000
            ratio = row["mean_ns"] / baseline
            figure.add_trace(go.Bar(
                x=[mean], y=[label(name)], orientation="h", name=label(name),
                marker_color=colors[name], legendgroup=name, showlegend=index == 1,
                legendrank=list(colors).index(name),
                error_x={"type": "data", "symmetric": False,
                         "array": [(row["upper_ns"] - row["mean_ns"]) / 1000],
                         "arrayminus": [(row["mean_ns"] - row["lower_ns"]) / 1000]},
                customdata=[[row["lower_ns"] / 1000, row["upper_ns"] / 1000,
                             row["sample_count"], ratio]],
                hovertemplate=("<b>%{fullData.name}</b><br>Mean: %{x:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
                               "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples<br>"
                               "%{customdata[3]:.3f}× kth mean time<extra></extra>"),
            ), row=index, col=1)
        figure.update_yaxes(row=index, col=1, autorange="reversed", categoryorder="array",
                            categoryarray=[label(r["algorithm"]) for r in points])
        figure.update_xaxes(row=index, col=1, title_text="Mean time per operation (µs) — lower is faster")
    style(figure, title + (" · SHORT RUN" if short else ""), 370 * len(cases) + 120)
    figure.update_layout(barmode="overlay", margin={"l": 235})
    scale_buttons(figure, tuple("xaxis" + (str(i) if i > 1 else "") for i in range(1, len(cases) + 1)))
    highlight_buttons(figure)
    return figure
