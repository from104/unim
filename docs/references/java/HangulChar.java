/**
 * 한글 음절 조합 및 분해 (유니코드 5.2)
 * 
 * @author "KiHyeon Seo" <from104@gmail.com>
 * @version 0.0.1
 */

package com.seojin.hangul;

public class HangulChar extends HangulJamo {
	// 초성,중성,종성 갯수
	public final int CHOSEONG_NUMBER = 19;
	public final int JUNGSEONG_NUMBER = 21;
	public final int JONGSEONG_NUMBER = 28;

	// 한글 음절 유니코드 시작 코드
	public final char SYLLABLE_BASE = 0xac00;
	// 한글 음절 갯수
	public final char SYLLABLE_NUMBER = CHOSEONG_NUMBER * JUNGSEONG_NUMBER
			* JONGSEONG_NUMBER;

	// 초,중,종성
	private Cho choseong = null;
	private Jung jungseong = null;
	private Jong jongseong = null;

	/**
	 * 한글 생성자
	 */
	public HangulChar() { }

	/**
	 * 한글 생성자 (초종중성 객체로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public HangulChar(Cho cho, Jung jung, Jong jong) {
		setJamo(cho, jung, jong);
	}

	/**
	 * 한글 생성자 (초종중성 순서로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public HangulChar(int cho, int jung, int jong) {
		setJamo(cho, jung, jong);
	}

	/**
	 * 한글 생성자 (초종중성 객체 이름으로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public HangulChar(String cho, String jung, String jong) {
		setJamo(cho, jung, jong);
	}

	/**
	 * 한글 생성자 (한글 음절로)
	 * @param syllable
	 */
	public HangulChar(char syllable) {
		setJamoBySyllable(syllable);
	}

	/**
	 * 한글 자모 정하기 (초종중성 객체로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public boolean setJamo(Cho cho, Jung jung, Jong jong) {
		boolean b = setCho(cho); b &= setJung(jung); b &= setJong(jong);
		return b;
	}

	/**
	 * 한글 자모 정하기 (초종중성 순서로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public boolean setJamo(int cho, int jung, int jong) {
		boolean b = setCho(cho); b &= setJung(jung); b &= setJong(jong);
		return b;
	}

	/**
	 * 한글 자모 정하기 (초종중성 객체 이름으로)
	 * @param cho
	 * @param jung
	 * @param jong
	 */
	public boolean setJamo(String cho, String jung, String jong) {
		boolean b = setCho(cho); b &= setJung(jung); b &= setJong(jong);
		return b;
	}

	/**
	 * 한글 자모 정하기 (한글 음절로)
	 * @param syllable
	 */
	public void setJamoBySyllable(char syllable) {
		if (syllable >= SYLLABLE_BASE
				&& syllable < SYLLABLE_BASE + SYLLABLE_NUMBER) {
			int cho, jung, jong, syll = syllable - SYLLABLE_BASE;

			jong = syll % JONGSEONG_NUMBER;
			jung = (syll - jong) / JONGSEONG_NUMBER % JUNGSEONG_NUMBER;
			cho = (syll - jong - jung * JUNGSEONG_NUMBER)
					/ (JUNGSEONG_NUMBER * JONGSEONG_NUMBER);

			setCho(cho);
			setJung(jung);
			setJong(jong);
		} else
			throw new IllegalArgumentException("한글 코드를 벗어났음");
	}

	/**
	 * 초성 정하기 (객체로)
	 * @param cho
	 */
	public boolean setCho(Cho cho) {
		if (cho != null) {
			choseong = cho;
			return true;
		} else {
			clearCho();
			return false;
		}
	}

	/**
	 * 중성 정하기 (객체로)
	 * @param jung
	 */
	public boolean setJung(Jung jung) {
		if (jung != null) {
			jungseong = jung;
			return true;
		} else {
			clearJung();
			return false;
		}
	}

	/**
	 * 종성 정하기 (객체로)
	 * @param jong
	 */
	public boolean setJong(Jong jong) {
		if (jong != null) {
			jongseong = jong;
			return true;
		} else {
			clearJong();
			return false;
		}
	}

	/**
	 * 초성 정하기 (순서로)
	 * @param cho
	 */
	public boolean setCho(int cho) {
		if (cho >= 0 && cho < JUNGSEONG_NUMBER) {
			setCho(getChoBySequence(cho));
			return true;
		} else {
			clearCho();
			return false;
		}
	}

	/**
	 * 중성 정하기 (순서로)
	 * @param jung
	 */
	public boolean setJung(int jung) {
		if (jung >= 0 && jung < JUNGSEONG_NUMBER) {
			setJung(getJungBySequence(jung));
			return true;
		} else {
			clearJong();
			return false;
		}
	}

	/**
	 * 종성 정하기 (순서로)
	 * @param jong
	 */
	public boolean setJong(int jong) {
		if (jong >= 0 && jong < JONGSEONG_NUMBER) {
			setJong(getJongBySequence(jong));
			return true;
		} else {
			jongseong = null;
			return false;
		}
	}

	/**
	 * 초성 정하기 (객체 이름으로)
	 * @param cho
	 */
	public boolean setCho(String cho) {
		try {
			setCho(Cho.valueOf(cho.toUpperCase()));
			return true;
		} catch (Exception e) {
			clearCho();
			return false;
		}
	}

	/**
	 * 중성 정하기 (객체 이름으로)
	 * @param jung
	 */
	public boolean setJung(String jung) {
		try {
			setJung(Jung.valueOf(jung.toUpperCase()));
			return true;
		} catch (Exception e) {
			clearJung();
			return false;
		}
	}

	/**
	 * 종성 정하기 (객체 이름으로)
	 * @param jong
	 */
	public boolean setJong(String jong) {
		try {
			setJong(Jong.valueOf(jong.toUpperCase()));
			return true;
		} catch (Exception e) {
			clearJong();
			return false;
		}
	}

	/**
	 * 초성 지우기
	 */
	public void clearCho() {
		choseong = null;
	}

	/**
	 * 중성 지우기
	 */
	public void clearJung() {
		jungseong = null;
	}

	/**
	 * 종성 지우기
	 */
	public void clearJong() {
		jongseong = null;
	}

	/**
	 * 자모 모두 지우기
	 */
	public void clear() {
		clearCho();
		clearJung();
		clearJong();
	}

	/**
	 * 초성이 있는지 여부
	 * @return
	 */
	public boolean isFilledCho() {
		return choseong != null;
	}

	/**
	 * 중성이 있는지 여부
	 * @return
	 */
	public boolean isFilledJung() {
		return jungseong != null;
	}

	/**
	 * 종성이 있는지 여부
	 * @return
	 */
	public boolean isFilledJong() {
		return jongseong != null;
	}

	/**
	 * 초성만 있는지 여부
	 * @return
	 */
	public boolean isFilledOnlyCho() {
		return isFilledCho() && !isFilledJung() && !isFilledJong();
	}

	/**
	 * 중성만 있는지 여부
	 * @return
	 */
	public boolean isFilledOnlyJung() {
		return !isFilledCho() && isFilledJung() && !isFilledJong();
	}

	/**
	 * 종성만 있는지 여부
	 * @return
	 */
	public boolean isFilledOnlyJong() {
		return !isFilledCho() && !isFilledJung() && isFilledJong();
	}

	/**
	 * 자모가 다 비었는지 여부
	 * @return
	 */
	public boolean isEmpty () {
		return !isFilledCho() && !isFilledJung() && !isFilledJong();
	}
	
	/**
	 * 초성 얻기
	 * @return
	 */
	public Cho getCho() {
		return choseong;
	}

	/**
	 * 중성 얻기
	 * @return
	 */
	public Jung getJung() {
		return jungseong;
	}

	/**
	 * 종성 얻기
	 * @return
	 */
	public Jong getJong() {
		return jongseong;
	}

	/**
	 * 초성 유니코드(첫가끝) 얻기
	 * @return
	 */
	public char getChoUnicode() {
		if (isFilledCho())
			return choseong.getUnicode();
		else
			return 0;
	}

	/**
	 * 중성 유니코드(첫가끝) 얻기
	 * @return
	 */
	public char getJungUnicode() {
		if (isFilledJung())
			return jungseong.getUnicode();
		else
			return 0;
	}

	/**
	 * 종성 유니코드(첫가끝) 얻기
	 * @return
	 */
	public char getJongUnicode() {
		if (isFilledJong() && jongseong.getSequence() != 0)
			return jongseong.getUnicode();
		else
			return 0;
	}

	/**
	 * 초성 유니코드(호환용) 얻기
	 * @return
	 */
	public char getChoUnicodeCompat() {
		if (isFilledCho())
			return choseong.getUnicodeCompat();
		else
			return 0;
	}

	/**
	 * 중성 유니코드(호환용) 얻기
	 * @return
	 */
	public char getJungUnicodeCompat() {
		if (isFilledJung())
			return jungseong.getUnicodeCompat();
		else
			return 0;
	}

	/**
	 * 종성 유니코드(호환용) 얻기
	 * @return
	 */
	public char getJongUnicodeCompat() {
		if (isFilledJong() && jongseong.getSequence() != 0)
			return jongseong.getUnicodeCompat();
		else
			return 0;
	}

	/**
	 * 한글 첫가끝 조합된 유니코드 얻기
	 * @return 첫가끝 문자 배열
	 */
	public char[] getUnicodes () {
		if (isEmpty())
			throw new NullPointerException("자모가 비었음");
		else if	(isFilledCho() && !isFilledJung() 
				               && isFilledJong()) // 초성,종성으로 조합되어있으면
			return new char[]{'?'};
		else if (!isFilledJong() || jongseong.equals(Jong.E)) { // 종성이 없으면
			char[] c = new char[2];
			if (isFilledCho())
				c[0] = choseong.getUnicode();
			else
				c[0] = Cho.F.getUnicode();
			if (isFilledJung())
				c[1] = jungseong.getUnicode();
			else
				c[1] = Jung.F.getUnicode();
			
			return c;
		} else {
			char[] c = new char[3];
			if (isFilledCho())
				c[0] = choseong.getUnicode();
			else
				c[0] = Cho.F.getUnicode();
			if (isFilledJung())
				c[1] = jungseong.getUnicode();
			else
				c[1] = Jung.F.getUnicode();
			c[2] = jongseong.getUnicode();
			c.toString();
			return c;			
		}
	}
	
	/**
	 * 한글 음절 얻기
	 * @return 한글 음절 또는 호환용 자모
	 */
	public char getSyllable() {
		if (isEmpty()) {
			throw new NullPointerException("자모가 비었음");
		} else if (isFilledOnlyCho()) {
			return getChoUnicodeCompat();
		} else if (isFilledOnlyJung()) {
			return getJungUnicodeCompat();
		} else if (isFilledOnlyJong()) {
			return getJongUnicodeCompat();
		} else if (isFilledCho() && isFilledJung()) {
			int choseong = this.choseong.getSequence();
			int jungseong = this.jungseong.getSequence();
			int jongseong = isFilledJong() ? this.jongseong.getSequence() : 0;

			return (char) (SYLLABLE_BASE + 
					choseong * JUNGSEONG_NUMBER	* JONGSEONG_NUMBER + 
					jungseong * JONGSEONG_NUMBER + 
					jongseong);
		} else {
			return '?';
		}
	}

	/**
	 * 조합된 한글을 문자열 변환
	 * @return 한글 음절 또는 첫가끝 코드 문자열
	 */
	@Override
	public String toString() {
		char h = getSyllable();
		if (h != '?')
			return  String.valueOf(h);
		else
			return getUnicodes().toString();
		
	}
}
