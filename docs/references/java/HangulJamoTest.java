package com.seojin.hangul;

public class HangulJamoTest extends HangulJamo {

	/**
	 * @param args
	 */
	public static void main(String[] args) {
		// TODO Auto-generated method stub
		Object o = null;
		if (isCho(o))
			System.out.println("초성 맞고요.");
		if (isJung(o)) 
			System.out.println("중성 맞고요.");
		if (isJong(o)) 
			System.out.println("종성 맞고요.");
		if (isJamo(o)) 
			System.out.println("자모 맞고요.");
		
	}

}