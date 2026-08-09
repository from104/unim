package com.seojin.hangul;
import java.io.IOException;
import java.util.HashMap;


public class HangulBuilder2BulTest extends HangulJamo {

	/**
	 * @param args
	 */
	public static void main(String[] args) throws IOException {
		HashMap<Character,Object> km2 = new HashMap<Character,Object>();
		int c=0;
		HangulChar h;
		HangulBuilder2Bul hb2 = new HangulBuilder2Bul();
		
		km2.put('q', Cho.B);
		km2.put('w', Cho.J);
		km2.put('e', Cho.D);
		km2.put('r', Cho.G);
		km2.put('t', Cho.S);
		km2.put('y', Jung.YO);
		km2.put('u', Jung.YEO);
		km2.put('i', Jung.YA);
		km2.put('o', Jung.AE);
		km2.put('p', Jung.E);
		km2.put('a', Cho.M);
		km2.put('s', Cho.N);
		km2.put('d', Cho.E);
		km2.put('f', Cho.R);
		km2.put('g', Cho.H);
		km2.put('h', Jung.O);
		km2.put('j', Jung.EO);
		km2.put('k', Jung.A);
		km2.put('l', Jung.I);
		km2.put('z', Cho.K);
		km2.put('x', Cho.T);
		km2.put('c', Cho.C);
		km2.put('v', Cho.P);
		km2.put('b', Jung.YU);
		km2.put('n', Jung.U);
		km2.put('m', Jung.EU);
		
		km2.put('Q', Cho.BB);
		km2.put('W', Cho.JJ);
		km2.put('E', Cho.DD);
		km2.put('R', Cho.GG);
		km2.put('T', Cho.SS);
		km2.put('Y', Jung.YO);
		km2.put('U', Jung.YEO);
		km2.put('I', Jung.YA);
		km2.put('O', Jung.YAE);
		km2.put('P', Jung.YE);
		km2.put('A', Cho.M);
		km2.put('S', Cho.N);
		km2.put('D', Cho.E);
		km2.put('F', Cho.R);
		km2.put('G', Cho.H);
		km2.put('H', Jung.O);
		km2.put('J', Jung.EO);
		km2.put('K', Jung.A);
		km2.put('L', Jung.I);
		km2.put('Z', Cho.K);
		km2.put('X', Cho.T);
		km2.put('C', Cho.C);
		km2.put('V', Cho.P);
		km2.put('B', Jung.YU);
		km2.put('N', Jung.U);
		km2.put('M', Jung.EU);
		
		do {
			c = System.in.read();
			if (km2.containsKey( ((char) c))) {
				h = hb2.addJamo(km2.get( ((char) c)));
				if (h != null) {
					System.out.print(h);
				}
			} else {
				if (hb2.isBuild()) {
					h = hb2.forceBuildHangul();
					System.out.print(h);
				}
				System.out.print((char) c);
			}
		} while (c != '.'); 
	}

}
