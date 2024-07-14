/**
 * 한글 자모 열거 상수 정의
 * @author "KiHyeon Seo" <from104@gmail.com>
 * @version 0.0.1
 */
package com.seojin.hangul;

public class HangulJamo {
	/**
	 * 한글 자모 인터페이스
	 */
	private interface Jamo {
		/**
		 * 음절 조합을 위한 순서 얻기
		 */
		public int getSequence() ;
		
		/**
		 * 유니코드(첫가끝) 얻기
		 */
		public char getUnicode () ;
		
		/**
		 * 유니코드(호환용 자모) 얻기
		 */
		public char getUnicodeCompat() ;
	}
	
	/**
	 * 초성 상수
	 */
	public enum Cho implements Jamo {
		// 상수 데이터
		F( -1, '\u115f', '\u3164'), // 초성 채움
		G(  0, '\u1100', '\u3131'), // ㄱ
		GG( 1, '\u1101', '\u3132'), // ㄲ
		N(  2, '\u1102', '\u3134'), // ㄴ
		D(  3, '\u1103', '\u3137'), // ㄷ
		DD( 4, '\u1104', '\u3138'), // ㄸ
		R(  5, '\u1105', '\u3139'), // ㄹ 
		M(  6, '\u1106', '\u3141'), // ㅁ 
		B(  7, '\u1107', '\u3142'), // ㅂ 
		BB( 8, '\u1108', '\u3143'), // ㅃ 
		S(  9, '\u1109', '\u3145'), // ㅅ 
		SS(10, '\u110a', '\u3146'), // ㅆ 
		E( 11, '\u110b', '\u3147'), // ㅇ 
		J( 12, '\u110c', '\u3148'), // ㅈ 
		JJ(13, '\u110d', '\u3149'), // ㅉ 
		C( 14, '\u110e', '\u314a'), // ㅊ 
		K( 15, '\u110f', '\u314b'), // ㅋ 
		T( 16, '\u1110', '\u314c'), // ㅌ 
		P( 17, '\u1111', '\u314d'), // ㅍ 
		H( 18, '\u1112', '\u314e'); // ㅎ 
			
		// 음절 조합용 순서
		private final int sequence;
		// 유니코드(첫가끝)
		private final char unicode;
		// 유니코드(호환용 자모)
		private final char unicodeCompat;
		
		/**
		 * 초성 상수 생성자
		 */
		Cho (int seq, char uni, char uniCompat) {
			sequence = seq;
			unicode = uni;
			unicodeCompat = uniCompat;
		}

		/**
		 * 음절 조합을 위한 순서 얻기
		 */
		@Override
		public int getSequence() {
			return sequence;
		}
		
		/**
		 * 유니코드(첫가끝) 얻기
		 */
		@Override
		public char getUnicode () {
			return unicode;
		}
		
		/**
		 * 유니코드(호환용 자모) 얻기
		 */
		@Override
		public char getUnicodeCompat() {
			return unicodeCompat;
		}

		/**
		 * 초성을 종성으로 변환
		 * @throws IllegalArgumentException
		 */
		public Jong toJong () throws IllegalArgumentException {
			if (this.name() == "E") 
				return Jong.NG;
			else if (this.name() == "R") 
				return Jong.L;
			else
				return Jong.valueOf(this.name());
		}
	}
	
	/**
	 * 중성 상수
	 */	
	public enum Jung implements Jamo {
		// 상수 데이터
		F(  -1, '\u1160', '\u3164'), // 중성 채움
		A(   0, '\u1161', '\u314f'), // ㅏ
		AE(  1, '\u1162', '\u3150'), // ㅐ
		YA(  2, '\u1163', '\u3151'), // ㅑ
		YAE( 3, '\u1164', '\u3152'), // ㅒ
		EO(  4, '\u1165', '\u3153'), // ㅓ
		E(   5, '\u1166', '\u3154'), // ㅔ
		YEO( 6, '\u1167', '\u3155'), // ㅕ
		YE(  7, '\u1168', '\u3156'), // ㅖ
		O(   8, '\u1169', '\u3157'), // ㅗ
		WA(  9, '\u116a', '\u3158'), // ㅘ
		WAE(10, '\u116b', '\u3159'), // ㅙ
		OE( 11, '\u116c', '\u315a'), // ㅚ
		YO( 12, '\u116d', '\u315b'), // ㅛ
		U(  13, '\u116e', '\u315c'), // ㅜ
		WEO(14, '\u116f', '\u315d'), // ㅝ
		WE( 15, '\u1170', '\u315e'), // ㅞ
		WI( 16, '\u1171', '\u315f'), // ㅟ
		YU( 17, '\u1172', '\u3160'), // ㅠ
		EU( 18, '\u1173', '\u3161'), // ㅡ
		YI( 19, '\u1174', '\u3162'), // ㅢ
		I(  20, '\u1175', '\u3163'); // ㅣ
		
		// 음절 조합용 순서
		private final int sequence;
		// 유니코드(첫가끝)
		private final char unicode;
		// 유니코드(호환용 자모)
		private final char unicodeCompat;
		
		/**
		 * 중성 상수 생성자
		 */
		Jung (int seq, char uni, char uniCompat) {
			sequence = seq;
			unicode = uni;
			unicodeCompat = uniCompat;
		}

		/**
		 * 음절 조합을 위한 순서 얻기
		 */
		@Override
		public int getSequence() {
			return sequence;
		}
		
		/**
		 * 유니코드(첫가끝) 얻기
		 */
		@Override
		public char getUnicode () {
			return unicode;
		}
		
		/**
		 * 유니코드(호환용 자모) 얻기
		 */
		@Override
		public char getUnicodeCompat() {
			return unicodeCompat;
		}
	}
	
	/**
	 * 종성 상수
	 */
	public enum Jong implements Jamo {
		// 상수 데이터
		E(  0, '\u0000', '\u0000'), // 종성 비움
		G(  1, '\u11a8', '\u3131'), // ㄱ
		GG( 2, '\u11a9', '\u3132'), // ㄲ
		GS( 3, '\u11aa', '\u3133'), // ㄳ
		N(  4, '\u11ab', '\u3134'), // ㄴ
		NJ( 5, '\u11ac', '\u3135'), // ㄵ
		NH( 6, '\u11ad', '\u3136'), // ㄶ
		D(  7, '\u11ae', '\u3137'), // ㄷ
		L(  8, '\u11af', '\u3139'), // ㄹ
		LG( 9, '\u11b0', '\u313a'), // ㄺ
		LM(10, '\u11b1', '\u313b'), // ㄻ
		LB(11, '\u11b2', '\u313c'), // ㄼ
		LS(12, '\u11b3', '\u313d'), // ㄽ
		LT(13, '\u11b4', '\u313e'), // ㄾ
		LP(14, '\u11b5', '\u313f'), // ㄿ
		LH(15, '\u11b6', '\u3140'), // ㅀ
		M( 16, '\u11b7', '\u3141'), // ㅁ
		B( 17, '\u11b8', '\u3142'), // ㅂ
		BS(18, '\u11b9', '\u3144'), // ㅄ
		S( 19, '\u11ba', '\u3145'), // ㅅ
		SS(20, '\u11bb', '\u3146'), // ㅆ
		NG(21, '\u11bc', '\u3147'), // ㅇ
		J( 22, '\u11bd', '\u3148'), // ㅈ
		C( 23, '\u11be', '\u314a'), // ㅊ
		K( 24, '\u11bf', '\u314b'), // ㅋ
		T( 25, '\u11c0', '\u314c'), // ㅌ
		P( 26, '\u11c1', '\u314d'), // ㅍ
		H( 27, '\u11c2', '\u314e'); // ㅎ
		
		// 음절 조합용 순서
		private final int sequence;
		// 유니코드(첫가끝)
		private final char unicode;
		// 유니코드(호환용 자모)
		private final char unicodeCompat;
		
		/**
		 * 중성 상수 생성자
		 */
		Jong (int seq, char uni, char uniCompat) {
			sequence = seq;
			unicode = uni;
			unicodeCompat = uniCompat;
		}

		/**
		 * 음절 조합을 위한 순서 얻기
		 */
		@Override
		public int getSequence() {
			return sequence;
		}
		
		/**
		 * 유니코드(첫가끝) 얻기
		 */
		@Override
		public char getUnicode () {
			return unicode;
		}

		/**
		 * 유니코드(호환용 자모) 얻기
		 */
		@Override
		public char getUnicodeCompat() {
			return unicodeCompat;
		}
		
		/**
		 * 종성을 초성으로 변환
		 * @throws IllegalArgumentException
		 */
		public Cho toCho  () throws IllegalArgumentException {
			if (this.name() == "NG") 
				return Cho.E;
			else if (this.name() == "L") 
				return Cho.R;
			else
				return Cho.valueOf(this.name());
		}
	}
	
	/** 
	 * 객체가 초성인지 판별
	 */
	public static boolean isCho (Object o) {
		return o instanceof Cho;
	}
	
	/** 
	 * 객체가 중성인지 판별
	 */
	public static boolean isJung (Object o) {
		return o instanceof Jung;
	}
	
	/** 
	 * 객체가 종성인지 판별
	 */
	public static boolean isJong (Object o) {
		return o instanceof Jong;
	}
	
	/** 
	 * 객체가 자모인지 판별
	 */
	public static boolean isJamo (Object o) {
		return o instanceof Jamo;
	}
	
	/**
	 * 순서로 초성 얻기
	 */
	public static Cho getChoBySequence (int seq) {
		for (Cho o : Cho.values()) {
			if (o.getSequence() == seq)
				return o;
		}
		return null;
	}
	
	/**
	 * 순서로 중성 얻기
	 */
	public static Jung getJungBySequence (int seq) {
		for (Jung o : Jung.values()) {
			if (o.getSequence() == seq)
				return o;
		}
		return null;
	}
	
	/**
	 * 순서로 종성 얻기
	 */
	public static Jong getJongBySequence (int seq) {
		for (Jong o : Jong.values()) {
			if (o.getSequence() == seq)
				return o;
		}
		return null;
	}
}
