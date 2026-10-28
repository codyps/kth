"""Interactive views of the same timing estimates used by the static exports."""

import plotly.graph_objects as go
from plotly.offline import get_plotlyjs
from plotly.subplots import make_subplots


def write_assets(output):
    # One local bundle for all charts; downloaded reports also work offline.
    (output / "plotly.min.js").write_text(get_plotlyjs(), encoding="utf-8")


def chart_html(figure, name):
    return figure.to_html(
        full_html=False, include_plotlyjs=False, div_id=name,
        config={"responsive": True, "scrollZoom": True, "displaylogo": False,
                "toImageButtonOptions": {"format": "svg", "filename": name}},
    )


def label(name):
    return name + (" (full sort)" if name.startswith("sort") else "")


def style(figure, title, height):
    figure.update_layout(
        title={"text": title, "x": 0.04}, height=height, autosize=True,
        paper_bgcolor="#f6f8fb", plot_bgcolor="white",
        font={"family": "Arial, sans-serif", "color": "#172941"},
        margin={"l": 100, "r": 40, "t": 120, "b": 120},
        legend={"orientation": "h", "y": -0.2, "groupclick": "togglegroup"},
        hoverlabel={"namelength": -1}, dragmode="zoom",
    )
    figure.update_xaxes(gridcolor="#dce3ed", zeroline=False, automargin=True)
    figure.update_yaxes(gridcolor="#dce3ed", zeroline=False, automargin=True)


def scale_buttons(figure, xaxes, yaxes=(), linear=True):
    def changes(kind):
        return {key: value for axis in (*xaxes, *yaxes)
                for key, value in ((f"{axis}.type", kind), (f"{axis}.autorange", True))}

    figure.update_layout(updatemenus=[{
        "type": "buttons", "direction": "right", "x": 1, "xanchor": "right", "y": 1.06,
        "showactive": True, "active": 1 if linear else 0,
        "buttons": [
            {"label": "Log axes" if yaxes else "Log time", "method": "relayout", "args": [changes("log")]},
            {"label": "Linear axes" if yaxes else "Linear time", "method": "relayout", "args": [changes("linear")]},
        ],
    }])


def scaling_figure(rows, title, colors, short=False, linear=False):
    figure = go.Figure()
    baseline = {r["n"]: r["mean_ns"] for r in rows if r["algorithm"] == "kth"}
    for name, color in colors.items():
        points = sorted((r for r in rows if r["algorithm"] == name), key=lambda r: r["n"])
        if not points:
            continue
        x = [r["n"] for r in points]
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
                  "dash": "dash" if name.startswith("sort") else "solid"},
            customdata=[[r["lower_ns"] / 1000, r["upper_ns"] / 1000,
                         r["sample_count"], r["mean_ns"] / baseline[r["n"]]] for r in points],
            hovertemplate=("n = %{x:,}<br>Mean: %{y:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
                           "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples<br>"
                           "%{customdata[3]:.3f}× kth mean time<extra>%{fullData.name}</extra>"),
        ))
    style(figure, title + (" · SHORT RUN" if short else ""), 640)
    ticks = sorted({r["n"] for r in rows})
    figure.update_xaxes(title_text="Number of elements n", type="linear" if linear else "log",
                        tickmode="array", tickvals=ticks, ticktext=[f"{n:,}" for n in ticks])
    figure.update_yaxes(title_text="Mean time per operation (µs) — lower is faster",
                        type="linear" if linear else "log", minorloglabels="none")
    scale_buttons(figure, ("xaxis",), ("yaxis",), linear=linear)
    return figure


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
                hovertemplate=("Mean: %{x:.4g} µs<br>95% CI: %{customdata[0]:.4g} – "
                               "%{customdata[1]:.4g} µs<br>%{customdata[2]} samples<br>"
                               "%{customdata[3]:.3f}× kth mean time<extra>%{fullData.name}</extra>"),
            ), row=index, col=1)
        figure.update_yaxes(row=index, col=1, autorange="reversed", categoryorder="array",
                            categoryarray=[label(r["algorithm"]) for r in points])
        figure.update_xaxes(row=index, col=1, title_text="Mean time per operation (µs) — lower is faster")
    style(figure, title + (" · SHORT RUN" if short else ""), 370 * len(cases) + 120)
    figure.update_layout(barmode="overlay", margin={"l": 180}, legend={"y": -0.07})
    scale_buttons(figure, tuple("xaxis" + (str(i) if i > 1 else "") for i in range(1, len(cases) + 1)))
    return figure
