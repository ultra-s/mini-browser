package dev.mini.browser

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.graphics.BlurMaskFilter
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.animation.DecelerateInterpolator
import android.widget.FrameLayout
import android.widget.LinearLayout
import kotlin.math.abs
import kotlin.math.min

/**
 * MiniUi — a fully custom-drawn UI toolkit. Zero native widgets: every button,
 * bar, tab, sheet, toast and switch is painted on Canvas with our ink/aurora design language.
 *
 * Palette:
 *   ink-0   #0b0a10  deep background
 *   ink-1   #14131b  surface
 *   ink-2   #1e1c27  raised surface
 *   glow-a  #ff7a45  ember (primary accent)
 *   glow-b  #b06cff  aurora (secondary accent)
 *   text    #ece9f4
 *   dim     #8b87a0
 */
object MiniUi {
    const val INK0 = 0xFF0B0A10.toInt()
    const val INK1 = 0xFF14131B.toInt()
    const val INK2 = 0xFF1E1C27.toInt()
    const val GLOW_A = 0xFFFF7A45.toInt()
    const val GLOW_B = 0xFFB06CFF.toInt()
    const val TEXT = 0xFFECE9F4.toInt()
    const val DIM = 0xFF8B87A0.toInt()

    fun dp(c: Context, v: Float): Float = v * c.resources.displayMetrics.density

    fun rounded(c: Context, color: Int, radiusDp: Float): android.graphics.drawable.GradientDrawable {
        val d = android.graphics.drawable.GradientDrawable()
        d.setColor(color)
        d.cornerRadius = dp(c, radiusDp)
        return d
    }
}

/** Custom-drawn icon button with press glow. */
class MiniIconButton(
    context: Context,
    var glyph: String,
    var sizeDp: Float = 44f,
    var onClick: () -> Unit = {}
) : View(context) {

    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        textAlign = Paint.Align.CENTER
        typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
    }
    private var pressed = false
    private var pressT = 0f   // 0..1 glow

    init {
        val s = MiniUi.dp(context, sizeDp)
        layoutParams = LinearLayout.LayoutParams(s.toInt(), s.toInt())
    }

    override fun onDraw(canvas: Canvas) {
        val cx = width / 2f
        val cy = height / 2f
        val r = min(width, height) / 2f - MiniUi.dp(context, 3f)
        if (pressT > 0f) {
            paint.color = MiniUi.GLOW_B
            paint.alpha = (90 * pressT).toInt()
            canvas.drawCircle(cx, cy, r, paint)
        }
        textPaint.color = if (pressed) MiniUi.GLOW_A else MiniUi.TEXT
        textPaint.textSize = MiniUi.dp(context, sizeDp * 0.42f)
        val y = cy - (textPaint.descent() + textPaint.ascent()) / 2f
        canvas.drawText(glyph, cx, y, textPaint)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> { pressed = true; animateGlow(1f) }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (pressed && e.actionMasked == MotionEvent.ACTION_UP) {
                    pressed = false; animateGlow(0f); onClick()
                } else { pressed = false; animateGlow(0f) }
            }
        }
        return true
    }

    private fun animateGlow(target: Float) {
        animate().cancel()
        val start = pressT
        val va = android.animation.ValueAnimator.ofFloat(start, target)
        va.duration = 140
        va.interpolator = DecelerateInterpolator()
        va.addUpdateListener { pressT = it.animatedValue as Float; invalidate() }
        va.start()
    }
}

/** Custom bottom sheet, fully drawn: scrim + sliding rounded panel with rows. */
class MiniSheet(private val activity: Activity) : FrameLayout(activity) {

    class Row(val glyph: String, val label: String, val action: () -> Unit)

    private val panel = MiniSheetPanel(activity)
    private var rows: List<Row> = emptyList()
    private var title: String = ""

    init {
        visibility = GONE
        elevation = MiniUi.dp(activity, 24f)
        addView(panel, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT, Gravity.BOTTOM))
        setOnClickListener { dismiss() }
    }

    fun show(titleText: String, sheetRows: List<Row>) {
        title = titleText
        rows = sheetRows
        panel.notifyData()
        visibility = VISIBLE
        panel.translationY = panel.height.toFloat().coerceAtLeast(800f)
        panel.animate().translationY(0f).setDuration(220).setInterpolator(DecelerateInterpolator()).start()
        alpha = 0f
        animate().alpha(1f).setDuration(180).start()
    }

    fun dismiss() {
        panel.animate().translationY(panel.height.toFloat()).setDuration(180).withEndAction {
            visibility = GONE
        }.start()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        panel.updateSize(w, h)
    }

    private inner class MiniSheetPanel(context: Context) : View(context) {
        private val panelPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = MiniUi.INK1 }
        private val titlePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = MiniUi.TEXT; textSize = MiniUi.dp(context, 16f)
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        }
        private val glyphPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER; textSize = MiniUi.dp(context, 19f)
        }
        private val labelPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = MiniUi.TEXT; textSize = MiniUi.dp(context, 15.5f)
        }
        private val divider = Paint().apply { color = 0x22FFFFFF }

        private var touchRow = -1
        private val rowH = MiniUi.dp(activity, 54f)

        fun notifyData() { requestLayout(); invalidate() }
        fun updateSize(w: Int, h: Int) { layoutParams = LayoutParams(w, (rows.size * rowH + MiniUi.dp(activity, 76f)).toInt()) }

        override fun onDraw(canvas: Canvas) {
            val w = width.toFloat()
            val h = height.toFloat()
            val r = MiniUi.dp(activity, 24f)
            val rect = RectF(0f, 0f, w, h)
            val path = Path()
            path.addRoundRect(rect, floatArrayOf(r, r, r, r, 0f, 0f, 0f, 0f), Path.Direction.CW)
            canvas.drawPath(path, panelPaint)

            val padX = MiniUi.dp(activity, 20f)
            canvas.drawText(title, padX, MiniUi.dp(activity, 34f), titlePaint)

            var y = MiniUi.dp(activity, 52f)
            rows.forEachIndexed { i, row ->
                if (i == touchRow) {
                    val hl = Paint().apply { color = 0x18B06CFF }
                    canvas.drawRect(0f, y, w, y + rowH, hl)
                }
                glyphPaint.color = if (i % 2 == 0) MiniUi.GLOW_A else MiniUi.GLOW_B
                val gy = y + rowH / 2f - (glyphPaint.descent() + glyphPaint.ascent()) / 2f
                canvas.drawText(row.glyph, padX + MiniUi.dp(activity, 10f), gy, glyphPaint)
                val ly = y + rowH / 2f - (labelPaint.descent() + labelPaint.ascent()) / 2f
                canvas.drawText(row.label, padX + MiniUi.dp(activity, 48f), ly, labelPaint)
                if (i < rows.size - 1) canvas.drawRect(padX, y + rowH, w - padX, y + rowH + 1f, divider)
                y += rowH
            }
        }

        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(e: MotionEvent): Boolean {
            val y = e.y
            val first = MiniUi.dp(activity, 52f)
            val idx = ((y - first) / rowH).toInt()
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN -> if (idx in rows.indices) { touchRow = idx; invalidate(); return true }
                MotionEvent.ACTION_UP -> {
                    if (touchRow in rows.indices) {
                        val row = rows[touchRow]
                        touchRow = -1; invalidate(); dismiss()
                        row.action()
                    }
                    return true
                }
                MotionEvent.ACTION_CANCEL -> { touchRow = -1; invalidate(); return true }
            }
            return true
        }
    }
}

/** Custom drawn toast pill. */
class MiniToast(private val activity: Activity) {
    fun show(text: String) {
        val v = MiniToastView(activity, text)
        val lp = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL)
        lp.bottomMargin = MiniUi.dp(activity, 96f).toInt()
        (activity.window.decorView as? ViewGroup)?.addView(v, lp)
        v.play()
    }

    private class MiniToastView(context: Context, val msg: String) : View(context) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xEE1E1C27.toInt() }
        private val tp = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = MiniUi.TEXT; textSize = MiniUi.dp(context, 14f)
        }
        private var life = 1f

        init {
            val w = tp.measureText(msg) + MiniUi.dp(context, 36f)
            val h = MiniUi.dp(context, 40f)
            layoutParams = FrameLayout.LayoutParams(w.toInt(), h.toInt())
        }

        fun play() {
            alpha = 0f
            animate().alpha(1f).setDuration(150).withEndAction {
                postDelayed({ animate().alpha(0f).setDuration(250).withEndAction {
                    (parent as? ViewGroup)?.removeView(this@MiniToastView)
                }.start() }, 1900)
            }.start()
        }

        override fun onDraw(canvas: Canvas) {
            canvas.drawRoundRect(0f, 0f, width.toFloat(), height.toFloat(),
                height / 2f, height / 2f, paint)
            val y = height / 2f - (tp.descent() + tp.ascent()) / 2f
            canvas.drawText(msg, width / 2f - tp.measureText(msg) / 2f, y, tp)
        }
    }
}

/** Progress hairline drawn in accent gradient. */
class MiniProgress(context: Context) : View(context) {
    private val p = Paint(Paint.ANTI_ALIAS_FLAG)
    var fraction = 0f
        set(v) { field = v.coerceIn(0f, 1f); invalidate() }

    init { layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, MiniUi.dp(context, 2.5f).toInt()) }

    override fun onDraw(canvas: Canvas) {
        if (fraction <= 0f || fraction >= 1f) return
        p.shader = LinearGradient(0f, 0f, width.toFloat(), 0f,
            MiniUi.GLOW_A, MiniUi.GLOW_B, Shader.TileMode.CLAMP)
        canvas.drawRect(0f, 0f, width * fraction, height.toFloat(), p)
        p.shader = null
    }
}
