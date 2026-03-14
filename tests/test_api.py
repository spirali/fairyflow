"""Tests for the alsie Python DSL (preamble.py)."""


# ── helpers ────────────────────────────────────────────────────────────────


def kf(node_dict, step):
    """Return {prop: value} for a given step, unwrapping {value, transition} entries."""
    raw = node_dict["keyframes"].get(str(step), {})
    return {
        k: (v["value"] if isinstance(v, dict) and "value" in v else v)
        for k, v in raw.items()
    }


def kf_raw(node_dict, step):
    """Return the raw keyframe dict (with transition info) for a given step."""
    return node_dict["keyframes"].get(str(step), {})


# ── basic node creation ────────────────────────────────────────────────────


def test_rect_type(scene):
    scene["rect"]()
    nodes = scene["tree"]()["nodes"]
    assert len(nodes) == 1
    assert nodes[0]["type"] == "rect"


def test_circle_type(scene):
    scene["circle"]()
    nodes = scene["tree"]()["nodes"]
    assert nodes[0]["type"] == "circle"


def test_node_type(scene):
    scene["node"]()
    nodes = scene["tree"]()["nodes"]
    assert nodes[0]["type"] == "node"


def test_multiple_roots(scene):
    scene["rect"]()
    scene["rect"]()
    scene["circle"]()
    assert len(scene["tree"]()["nodes"]) == 3


# ── property setters ───────────────────────────────────────────────────────


def test_size(scene):
    scene["rect"]().size(30, 20)
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["width"] == 30
    assert kf(n, 0)["height"] == 20


def test_color(scene):
    scene["rect"]().color("green")
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["color"] == "green"


def test_radius(scene):
    scene["circle"]().radius(15)
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["radius"] == 15


def test_position(scene):
    scene["rect"]().position(10, 20)
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["x"] == 10
    assert kf(n, 0)["y"] == 20


# ── type-based property filtering ─────────────────────────────────────────


def test_rect_ignores_radius(scene):
    scene["rect"]().radius(10)
    n = scene["tree"]()["nodes"][0]
    assert "radius" not in kf(n, 0)


def test_circle_ignores_size(scene):
    scene["circle"]().size(100, 50)
    n = scene["tree"]()["nodes"][0]
    assert "width" not in kf(n, 0)
    assert "height" not in kf(n, 0)


# ── keyframes & steps ──────────────────────────────────────────────────────


def test_at_writes_to_correct_step(scene):
    scene["rect"]().color("green").at(1).color("red")
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["color"] == "green"
    assert kf(n, 1)["color"] == "red"


def test_steps_count(scene):
    scene["rect"]().at(2).color("blue")
    assert scene["tree"]()["steps"] == 3


def test_steps_default_is_one(scene):
    scene["rect"]()
    assert scene["tree"]()["steps"] == 1


def test_move_accumulates_position(scene):
    scene["rect"]().position(10, 20).at(1).move(5, -5)
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 1)["x"] == 15
    assert kf(n, 1)["y"] == 15


def test_move_from_zero_when_no_prior_position(scene):
    scene["rect"]().at(1).move(3, 4)
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 1)["x"] == 3
    assert kf(n, 1)["y"] == 4


# ── lifetime (at= / remove) ────────────────────────────────────────────────


def test_start_default_is_zero(scene):
    scene["rect"]()
    assert scene["tree"]()["nodes"][0]["start"] == 0


def test_at_param_sets_start(scene):
    scene["rect"](at=2)
    n = scene["tree"]()["nodes"][0]
    assert n["start"] == 2


def test_at_param_advances_time_cursor(scene):
    """Properties set after rect(at=2) should land in keyframe 2."""
    scene["rect"](at=2).color("blue")
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 2)["color"] == "blue"
    assert kf(n, 0) == {}


def test_remove_sets_end(scene):
    scene["rect"]().at(3).remove()
    n = scene["tree"]()["nodes"][0]
    assert n["end"] == 3


def test_remove_extends_steps(scene):
    scene["rect"]().at(3).remove()
    assert scene["tree"]()["steps"] == 4


def test_no_end_when_not_removed(scene):
    scene["rect"]()
    assert "end" not in scene["tree"]()["nodes"][0]


# ── nesting ────────────────────────────────────────────────────────────────


def test_children_added_inside_with(scene):
    with scene["node"]():
        scene["rect"]()
        scene["circle"]()
    root = scene["tree"]()["nodes"][0]
    assert len(root["children"]) == 2
    assert root["children"][0]["type"] == "rect"
    assert root["children"][1]["type"] == "circle"


def test_nested_not_in_roots(scene):
    with scene["node"]():
        scene["rect"]()
    assert len(scene["tree"]()["nodes"]) == 1


def test_deep_nesting(scene):
    with scene["node"]():
        with scene["node"]():
            scene["rect"]()
    inner = scene["tree"]()["nodes"][0]["children"][0]
    assert inner["children"][0]["type"] == "rect"


# ── group ──────────────────────────────────────────────────────────────────


def test_group_applies_color_to_all(scene):
    r1 = scene["rect"]()
    r2 = scene["rect"]()
    scene["group"]([r1, r2]).color("red")
    nodes = scene["tree"]()["nodes"]
    assert kf(nodes[0], 0)["color"] == "red"
    assert kf(nodes[1], 0)["color"] == "red"


def test_group_ignores_unsupported_prop(scene):
    r = scene["rect"]()
    c = scene["circle"]()
    scene["group"]([r, c]).radius(30)
    nodes = scene["tree"]()["nodes"]
    assert "radius" not in kf(nodes[0], 0)  # rect: ignored
    assert kf(nodes[1], 0)["radius"] == 30  # circle: applied


def test_group_at_sets_step_for_all(scene):
    r1 = scene["rect"]()
    r2 = scene["rect"]()
    scene["group"]([r1, r2]).at(2).color("blue")
    nodes = scene["tree"]()["nodes"]
    assert kf(nodes[0], 2)["color"] == "blue"
    assert kf(nodes[1], 2)["color"] == "blue"


def test_group_returns_self_for_chaining(scene):
    r1 = scene["rect"]()
    r2 = scene["rect"]()
    g = scene["group"]([r1, r2])
    assert g.color("red").at(1).color("blue") is g


# ── transitions ────────────────────────────────────────────────────────────


def test_default_transition_is_sharp(scene):
    scene["circle"]().radius(10)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 0)["radius"]["transition"] == "sharp"


def test_smooth_sets_transition(scene):
    scene["circle"]().radius(10).at(1).smooth().radius(20)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 1)["radius"]["transition"] == "smooth"


def test_sharp_sets_transition_explicitly(scene):
    scene["circle"]().radius(10).at(1).sharp().radius(20)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 1)["radius"]["transition"] == "sharp"


def test_smooth_does_not_affect_previous_keyframe(scene):
    scene["circle"]().radius(10).at(1).smooth().radius(20)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 0)["radius"]["transition"] == "sharp"


def test_transition_persists_across_setters(scene):
    """smooth() should apply to all setters that follow until changed."""
    scene["rect"]().color("red").at(1).smooth().color("blue").size(10, 20)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 1)["color"]["transition"] == "smooth"
    assert kf_raw(n, 1)["width"]["transition"] == "smooth"
    assert kf_raw(n, 1)["height"]["transition"] == "smooth"


def test_sharp_reverts_after_smooth(scene):
    scene["circle"]().radius(5).at(1).smooth().radius(10).at(2).sharp().radius(15)
    n = scene["tree"]()["nodes"][0]
    assert kf_raw(n, 1)["radius"]["transition"] == "smooth"
    assert kf_raw(n, 2)["radius"]["transition"] == "sharp"


def test_smooth_color_interpolation_stored(scene):
    scene["circle"]().color("#101010").at(1).smooth().color("#ff9090")
    n = scene["tree"]()["nodes"][0]
    assert kf(n, 0)["color"] == "#101010"
    assert kf(n, 1)["color"] == "#ff9090"
    assert kf_raw(n, 1)["color"]["transition"] == "smooth"
