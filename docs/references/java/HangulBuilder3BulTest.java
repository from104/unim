package com.seojin.hangul;

import java.io.IOException;
import java.util.HashMap;


public class HangulBuilder3BulTest extends HangulJamo {

	/**
	 * @param args
	 */
	public static void main(String[] args) throws IOException {
		HashMap<Character,Object> km3 = new HashMap<Character,Object>();
		
		km3.put('1', Jong.H);
		km3.put('2', Jong.SS);
		km3.put('3', Jong.B);
		km3.put('4', Jung.YO);
		km3.put('5', Jung.YU);
		km3.put('6', Jung.YA);
		km3.put('7', Jung.YE);
		km3.put('8', Jung.YI);
		km3.put('9', Jung.U);
		km3.put('0', Cho.K);
		km3.put('q', Jong.S);
		km3.put('w', Jong.L);
		km3.put('e', Jung.YEO);
		km3.put('r', Jung.AE);
		km3.put('t', Jung.EO);
		km3.put('y', Cho.R);
		km3.put('u', Cho.D);
		km3.put('i', Cho.M);
		km3.put('o', Cho.C);
		km3.put('p', Cho.P);
		km3.put('a', Jong.NG);
		km3.put('s', Jong.N);
		km3.put('d', Jung.I);
		km3.put('f', Jung.A);
		km3.put('g', Jung.EU);
		km3.put('h', Cho.N);
		km3.put('j', Cho.E);
		km3.put('k', Cho.G);
		km3.put('l', Cho.J);
		km3.put(';', Cho.B);
		km3.put('\'', Cho.T);
		km3.put('z', Jong.M);
		km3.put('x', Jong.G);
		km3.put('c', Jung.E);
		km3.put('v', Jung.O);
		km3.put('b', Jung.U);
		km3.put('n', Cho.S);
		km3.put('m', Cho.H);
		
//		km3.put('Q', Cho.BB);
//		km3.put('W', Cho.JJ);
//		km3.put('E', Cho.DD);
//		km3.put('R', Cho.GG);
//		km3.put('T', Cho.SS);
//		km3.put('Y', Jung.YO);
//		km3.put('U', Jung.YEO);
//		km3.put('I', Jung.YA);
//		km3.put('O', Jung.YAE);
//		km3.put('P', Jung.YE);
//		km3.put('A', Cho.M);
//		km3.put('S', Cho.N);
//		km3.put('D', Cho.E);
//		km3.put('F', Cho.R);
//		km3.put('G', Cho.H);
//		km3.put('H', Jung.O);
//		km3.put('J', Jung.EO);
//		km3.put('K', Jung.A);
//		km3.put('L', Jung.I);
//		km3.put('Z', Cho.K);
//		km3.put('X', Cho.T);
//		km3.put('C', Cho.C);
//		km3.put('V', Cho.P);
//		km3.put('B', Jung.YU);
//		km3.put('N', Jung.U);
//		km3.put('M', Jung.EU);
		
		int c=0;
		HangulChar h;
		HangulBuilder3Bul hb3 = new HangulBuilder3Bul();
		
		while (c != '.') {
			c = System.in.read();
			if (km3.containsKey( (char) c)) {
				h = hb3.addJamo(km3.get( (char) c));
				if (h != null) {
					System.out.print(h);
				}
			} else {
				if (hb3.isBuild()) {
					h = hb3.forceBuildHangul();
					System.out.print(h);
				}
				System.out.print((char) c);
			}
		} 
	}

}
