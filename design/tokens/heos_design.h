/*
 * heos_design.h — значения дизайна HeOS (источник: design/tokens/tokens.json).
 * Цвета интерфейса берите из темы через ui_color(); здесь — то, чего нет в color_which.
 */
#ifndef HEOS_DESIGN_H
#define HEOS_DESIGN_H

#include <GraphicsDefs.h>
#include <Polygon.h>
#include <Rect.h>

namespace heos {

const float kChamferSmall  = 5.0f;   // метки, поля, чекбоксы
const float kChamferMedium = 9.0f;   // кнопки, плитки
const float kChamferLarge  = 18.0f;  // окна, диалоги

const rgb_color kMedia     = {0xB0, 0x26, 0xFF, 255};  // медиа и ИИ
const rgb_color kTextMuted = {0x9A, 0xA3, 0xB2, 255};
const rgb_color kFocus     = {0x00, 0xE5, 0xFF, 255};
const float kFocusWidth = 3.0f, kFocusOffset = 3.0f;

// микросекунды; значения макета, подобрать на живой системе
const bigtime_t kMotionFast = 120000, kMotionBase = 200000, kMotionWindow = 280000;

// Срез 45° сверху слева и снизу справа, как у знака HeOS.
inline BPolygon ChamferedRect(BRect r, float c)
{
	BPoint p[6] = {
		BPoint(r.left + c, r.top), BPoint(r.right, r.top),
		BPoint(r.right, r.bottom - c), BPoint(r.right - c, r.bottom),
		BPoint(r.left, r.bottom), BPoint(r.left, r.top + c)
	};
	return BPolygon(p, 6);
}

} // namespace heos

#endif // HEOS_DESIGN_H
