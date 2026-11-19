// Orthographic attitude sphere in the local velocity frame. Grid meridians and
// parallels clip at the back of the ball; no bitmap pretending to be a gauge.
export function navball(canvas, pitch, yaw, roll) {
  const g = canvas.getContext("2d"),
    cx = 150,
    cy = 119,
    r = 101;
  g.clearRect(0, 0, 300, 240);
  g.save();
  g.translate(cx, cy);
  g.rotate(-roll);
  const project = (a, l) => {
    const y = Math.sin(l),
      s = Math.cos(l),
      z = s * Math.cos(a - yaw);
    return [
      r * s * Math.sin(a - yaw),
      -r * (y * Math.cos(pitch) - z * Math.sin(pitch)),
      y * Math.sin(pitch) + z * Math.cos(pitch),
    ];
  };
  function line(points, color, width = 1) {
    g.beginPath();
    let front = false;
    for (const p of points) {
      if (p[2] >= -0.001) {
        if (!front) g.moveTo(p[0], p[1]);
        else g.lineTo(p[0], p[1]);
        front = true;
      } else front = false;
    }
    g.strokeStyle = color;
    g.lineWidth = width;
    g.stroke();
  }
  const rad = Math.PI / 180;
  g.beginPath();
  g.arc(0, 0, r, 0, 7);
  g.fillStyle = "#193949";
  g.fill();
  g.save();
  g.clip();
  const horizon = Array.from({ length: 181 }, (_, i) =>
    project(yaw + (i - 90) * rad, 0),
  );
  g.beginPath();
  horizon.forEach((p, i) => (i ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1])));
  g.lineTo(r, r);
  g.lineTo(-r, r);
  g.closePath();
  g.fillStyle = "#664a35";
  g.fill();
  for (let l = -75; l <= 75; l += 15) {
    line(
      Array.from({ length: 121 }, (_, i) => project(i * 3 * rad, l * rad)),
      l === 0 ? "#c9a979" : "#80989877",
      l === 0 ? 2 : 1,
    );
  }
  for (let a = 0; a < 360; a += 30) {
    line(
      Array.from({ length: 61 }, (_, i) =>
        project(a * rad, (i * 3 - 90) * rad),
      ),
      "#a7b4af63",
    );
  }
  const grad = g.createRadialGradient(-25, -35, 10, 0, 0, r);
  grad.addColorStop(0, "#ffffff0a");
  grad.addColorStop(0.73, "#00000005");
  grad.addColorStop(1, "#02060980");
  g.fillStyle = grad;
  g.fillRect(-r, -r, 2 * r, 2 * r);
  g.font = "12px monospace";
  g.fillStyle = "#dce5dc";
  g.textAlign = "center";
  for (let a = 0; a < 360; a += 30) {
    const p = project(a * rad, 0.14);
    if (p[2] > 0.2)
      g.fillText(
        a === 0 ? "090" : String((a + 90) % 360).padStart(3, "0"),
        p[0],
        p[1],
      );
  }
  g.restore();
  g.beginPath();
  g.arc(0, 0, r + 4, 0, 7);
  g.strokeStyle = "#556069";
  g.lineWidth = 3;
  g.stroke();
  for (let a = 0; a < 360; a += 10) {
    g.save();
    g.rotate(a * rad);
    g.beginPath();
    g.moveTo(0, -r - 8);
    g.lineTo(0, -r - (a % 30 === 0 ? 17 : 12));
    g.strokeStyle = a % 30 === 0 ? "#b6b9b7" : "#687580";
    g.lineWidth = 2;
    g.stroke();
    g.restore();
  }
  g.rotate(roll);
  g.strokeStyle = "#ffc179";
  g.lineWidth = 4;
  g.beginPath();
  g.moveTo(-43, 0);
  g.lineTo(-12, 0);
  g.lineTo(-6, 6);
  g.lineTo(0, 0);
  g.lineTo(6, 6);
  g.lineTo(12, 0);
  g.lineTo(43, 0);
  g.stroke();
  g.fillStyle = "#ffc179";
  g.beginPath();
  g.moveTo(0, -r + 4);
  g.lineTo(-5, -r - 6);
  g.lineTo(5, -r - 6);
  g.fill();
  g.restore();
}
